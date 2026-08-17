# groupscape-web

- When you finish working on a change here, always verify the Docker builds succeed (`site/Dockerfile` and `server/Dockerfile`) and that tests pass (`npm test` and `npm run test:e2e` in `site/`, `cargo test` in `server/`) before reporting the work as done.
- For any UI or UX changes, use a Claude Artifact (via the `/prototype` skill) to get design confirmation before implementing — iterate on the artifact with the user until approved. Wait for the user's actual answer before proceeding; don't pick an option yourself and move on, even if the session otherwise runs unattended.
- Credentials in `.env` can be used to connect to and query the database directly.
- Any new env var must be added to `.env.example` and `.env.prod.example` (with a comment) in the same change that introduces it.
