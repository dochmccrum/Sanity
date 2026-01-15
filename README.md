# Sanite

Normal notes for normal people. Folders, markdown, flawless cross-platform and sync support and very easy self-hosting.

## Features

- 📝 **Rich Note Editing** - Markdown support with WYSIWYG editor
- 📁 **Folder Organization** - Organize notes into folders
- 🔐 **Secure Authentication** - JWT-based user authentication
- 🖥️ **Desktop & Web** - Available as a desktop app (Tauri) and web application
- 🔄 **Sync** - Keep your notes synchronized
- 🎨 **Modern UI** - Built with Svelte and Tailwind CSS
- 🐳 **Containerized** - Docker support for easy deployment

## Tech Stack

- **Frontend**: Svelte, SvelteKit, TypeScript, Tailwind CSS, Vite
- **Backend**: Rust, Axum web framework
- **Desktop**: Tauri
- **Database**: PostgreSQL with migrations
- **Authentication**: JWT
- **Deployment**: Docker

## Getting Started

### Prerequisites

- Node.js 16+ and npm/yarn
- Rust (for backend/desktop development)
- Docker (optional, for containerized deployment)

### Development

1. Install dependencies:
   ```sh
   npm install
   ```

2. Start the development server:
   ```sh
   npm run dev
   ```

   Or open in a new browser tab:
   ```sh
   npm run dev -- --open
   ```

3. For backend development, see [server/](server/) directory

### Building

Build for production:
```sh
npm run build
```

Preview the production build:
```sh
npm run preview
```

### Desktop App (Tauri)

Build the desktop application:
```sh
npm run tauri build
```

## Project Structure

```
├── src/                 # Frontend code (Svelte/TypeScript)
│   ├── components/      # Reusable UI components
│   ├── lib/             # Utilities, stores, and API adapters
│   ├── routes/          # SvelteKit pages
│   └── types/           # TypeScript type definitions
├── server/              # Rust backend
│   ├── src/
│   │   ├── api/         # API endpoints
│   │   ├── auth/        # Authentication logic
│   │   └── db/          # Database models
│   └── migrations/      # SQL migrations
└── src-tauri/           # Tauri desktop configuration
```

## Deployment

### Docker

Build and run the application in Docker:
```sh
docker-compose up
```

See [DEPLOYMENT.md](DEPLOYMENT.md) for detailed deployment instructions.

## Documentation

- [Branching Strategy](BRANCHING.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Image Support](IMAGE_SUPPORT.md)

## License

See [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please ensure your changes follow the project's branching strategy and testing requirements.
