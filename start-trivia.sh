#!/bin/bash

# Trivia App Complete Startup Script
# Starts both servers and exposes them for multiplayer access

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BACKEND_PORT=3001
FRONTEND_PORT=3000
USE_NGROK=false

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if a port is in use
port_in_use() {
    lsof -i :$1 >/dev/null 2>&1
}

# Function to kill processes on specific ports
kill_port() {
    local port=$1
    if port_in_use $port; then
        print_warning "Killing existing processes on port $port..."
        lsof -ti :$port | xargs kill -9 2>/dev/null || true
        sleep 2
    fi
}

# Function to get local network IP
get_local_ip() {
    if command_exists ipconfig; then
        # Windows
        ipconfig | grep "IPv4" | head -1 | awk '{print $14}'
    elif command_exists ifconfig; then
        # macOS/Linux
        ifconfig | grep "inet " | grep -v 127.0.0.1 | head -1 | awk '{print $2}'
    elif command_exists ip; then
        # Linux
        ip route get 1 | awk '{print $7}' | head -1
    else
        echo "Unable to determine local IP"
    fi
}

# Function to start backend server
start_backend() {
    print_status "Starting backend server..."
    cd backend
    
    # Create .env if it doesn't exist
    if [ ! -f .env ]; then
        print_warning "No .env file found!"
        print_status "You need a SHOPIFY_API_TOKEN for LLM-generated questions."
        print_status "Create backend/.env with: SHOPIFY_API_TOKEN=your_token_here"
        print_status "Or the game will use hardcoded fallback questions."
        echo "SHOPIFY_API_TOKEN=dummy_token_for_development" > .env
        print_warning "Created .env with dummy token - update it for LLM questions!"
    fi
    
    # Start backend bound to all interfaces for network access
    print_status "Backend will be accessible on all network interfaces (0.0.0.0:$BACKEND_PORT)"
    HOST=0.0.0.0 cargo run &
    BACKEND_PID=$!
    cd ..
    
    # Wait for backend to start
    print_status "Waiting for backend to start..."
    for i in {1..30}; do
        if curl -s http://localhost:$BACKEND_PORT/api/trivia-settings >/dev/null 2>&1; then
            print_success "Backend started successfully!"
            return 0
        fi
        sleep 1
    done
    
    print_error "Backend failed to start within 30 seconds"
    return 1
}

# Function to start frontend server
start_frontend() {
    print_status "Starting frontend server..."
    cd frontend
    
    # Start frontend bound to all interfaces for network access
    print_status "Frontend will be accessible on all network interfaces (0.0.0.0:$FRONTEND_PORT)"
    HOST=0.0.0.0 PORT=$FRONTEND_PORT npm start &
    FRONTEND_PID=$!
    cd ..
    
    # Wait for frontend to start
    print_status "Waiting for frontend to start..."
    for i in {1..60}; do
        if curl -s http://localhost:$FRONTEND_PORT >/dev/null 2>&1; then
            print_success "Frontend started successfully!"
            return 0
        fi
        sleep 1
    done
    
    print_error "Frontend failed to start within 60 seconds"
    return 1
}

# Function to setup ngrok tunneling
setup_ngrok() {
    if ! command_exists ngrok; then
        print_error "ngrok is not installed. Please install it from https://ngrok.com/"
        print_status "On macOS: brew install ngrok"
        return 1
    fi
    
    print_status "Setting up ngrok tunnels for both frontend and backend..."
    
    # Start ngrok for frontend
    ngrok http $FRONTEND_PORT &
    NGROK_FRONTEND_PID=$!
    
    # Start ngrok for backend  
    ngrok http $BACKEND_PORT &
    NGROK_BACKEND_PID=$!
    
    sleep 5
    
    # Get ngrok URLs
    if command_exists curl && command_exists jq; then
        FRONTEND_URL=$(curl -s http://localhost:4040/api/tunnels | jq -r '.tunnels[] | select(.config.addr | contains("'$FRONTEND_PORT'")) | .public_url' 2>/dev/null | head -1)
        BACKEND_URL=$(curl -s http://localhost:4041/api/tunnels | jq -r '.tunnels[] | select(.config.addr | contains("'$BACKEND_PORT'")) | .public_url' 2>/dev/null | head -1)
        
        if [ "$FRONTEND_URL" != "null" ] && [ "$FRONTEND_URL" != "" ]; then
            print_success "🌍 Public Frontend URL: $FRONTEND_URL"
        fi
        
        if [ "$BACKEND_URL" != "null" ] && [ "$BACKEND_URL" != "" ]; then
            print_success "🌍 Public Backend URL: $BACKEND_URL"
        fi
        
        print_success "✅ Both servers are now publicly accessible!"
        print_status "📤 Share the frontend URL with players: $FRONTEND_URL"
    else
        print_warning "Install jq for automatic URL display: brew install jq"
        print_status "Check ngrok dashboards:"
        print_status "  Frontend: http://localhost:4040"
        print_status "  Backend: http://localhost:4041"
    fi
}

# Function to cleanup on exit
cleanup() {
    print_status "Shutting down servers..."
    
    # Kill backend
    if [ ! -z "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null || true
    fi
    
    # Kill frontend
    if [ ! -z "$FRONTEND_PID" ]; then
        kill $FRONTEND_PID 2>/dev/null || true
    fi
    
    # Kill ngrok processes
    if [ ! -z "$NGROK_FRONTEND_PID" ]; then
        kill $NGROK_FRONTEND_PID 2>/dev/null || true
    fi
    
    if [ ! -z "$NGROK_BACKEND_PID" ]; then
        kill $NGROK_BACKEND_PID 2>/dev/null || true
    fi
    
    # Clean up any remaining processes
    kill_port $BACKEND_PORT
    kill_port $FRONTEND_PORT
    pkill -f "ngrok http" 2>/dev/null || true
    
    print_success "Cleanup completed"
    exit 0
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Starts trivia game servers and exposes them for multiplayer access."
    echo ""
    echo "Options:"
    echo "  --ngrok       Also create public internet tunnels"
    echo "  --help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0            # Start servers with local network access"
    echo "  $0 --ngrok    # Start servers + create public tunnels"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --ngrok)
            USE_NGROK=true
            shift
            ;;
        --help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Main script execution
main() {
    print_status "🎯 Starting Trivia Game Server..."
    echo ""
    
    # Setup signal handlers for cleanup
    trap cleanup SIGINT SIGTERM EXIT
    
    # Check prerequisites
    if ! command_exists cargo; then
        print_error "Rust/Cargo is not installed. Please install from https://rustup.rs/"
        exit 1
    fi
    
    if ! command_exists npm; then
        print_error "Node.js/npm is not installed. Please install from https://nodejs.org/"
        exit 1
    fi
    
    # Kill any existing processes on our ports
    kill_port $BACKEND_PORT
    kill_port $FRONTEND_PORT
    
    # Start backend server
    if ! start_backend; then
        print_error "Failed to start backend server"
        exit 1
    fi
    
    # Start frontend server
    if ! start_frontend; then
        print_error "Failed to start frontend server"
        exit 1
    fi
    
    # Setup ngrok if requested
    if [ "$USE_NGROK" = true ]; then
        setup_ngrok
    fi
    
    # Show access information
    echo ""
    print_success "🚀 Trivia Game Server is running!"
    echo ""
    
    print_status "🏠 Local Access:"
    echo "  Game URL: http://localhost:$FRONTEND_PORT"
    echo ""
    
    # Show network access info
    LOCAL_IP=$(get_local_ip)
    if [ "$LOCAL_IP" != "Unable to determine local IP" ] && [ ! -z "$LOCAL_IP" ]; then
        print_status "📶 Network Access (share with others on your WiFi):"
        echo "  Game URL: http://$LOCAL_IP:$FRONTEND_PORT"
        echo "  Backend: http://$LOCAL_IP:$BACKEND_PORT (auto-detected by frontend)"
        echo ""
    fi
    
    if [ "$USE_NGROK" = true ]; then
        print_status "🌍 Public Internet Access:"
        echo "  Check URLs above - share the frontend URL with anyone!"
        echo ""
    else
        print_status "💡 Want public internet access? Run: $0 --ngrok"
        echo ""
    fi
    
    print_status "🎮 Your computer is now the game server!"
    print_status "Players can join by visiting the game URL"
    print_status "Press Ctrl+C to stop the server"
    
    # Wait for processes
    wait
}

# Run main function
main "$@" 