@echo off
dotenv -e .env -- turbo run dev --env-mode=loose --ui tui %*