# System Manager Server

This is a web server for managing a Linux system. It provides a web interface for viewing system status, managing users, and updating the system.

## Getting Started

### Prerequisites

*   [Rust](https://www.rust-lang.org/)
*   [Node.js](https://nodejs.org/)
*   [Tailwind CSS](https://tailwindcss.com/)

### Installation

1.  Clone the repository:
    ```bash
    git clone https://github.com/your-username/system_manager_server.git
    ```
2.  Install the dependencies:
    ```bash
    npm install
    ```
3.  Build the server:
    ```bash
    cargo build
    ```
4.  Run the server:
    ```bash
    cargo run
    ```

## Usage

The server will be available at `http://localhost:8080`.

## Known Issues

*   The update manager is currently not implemented. The UI provides an interface for managing updates, but the backend logic is not yet complete.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.
