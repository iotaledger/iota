#!/bin/bash

# Deactivate the virtual environment if it was activated
cleanup() {
    if [ "$in_venv" = true ]; then
        echo "Deactivating virtual environment..."
        deactivate
    fi
}

# Set a trap to call cleanup on exit or error
trap cleanup EXIT

# Check if either "python" or "python3" exists and use it
if command -v python3 &>/dev/null; then
    PYTHON_CMD="python3"
elif command -v python &>/dev/null; then
    PYTHON_CMD="python"
else
    echo "Neither python nor python3 binary is installed. Please install Python."
    exit 1
fi

# Check if the "requirements.txt" file exists
if [ -f "requirements.txt" ]; then
    # Only enter a virtual environment if there are dependencies

    # Check if running in a virtual environment
    if [ -z "$VIRTUAL_ENV" ]; then
        echo "Not in a virtual environment. Activating..."

        # Check if the "venv" folder and venv/bin/activate exists
        if [ ! -f "venv/bin/activate" ]; then
            # Delete a potentially broken venv folder
            if [ -d "venv" ]; then
                echo "Deleting broken virtual environment..."
                rm -rf venv
            fi

            echo "Virtual environment doesn't exist yet, creating..."

            $PYTHON_CMD -m venv venv
            source venv/bin/activate

            echo "Installing packages..."
            pip install -r requirements.txt
        else
            source venv/bin/activate
            in_venv=true
        fi
    fi
fi
