# groupscape-web

- When you finish working on a change here, always verify the Docker builds succeed (`site/Dockerfile` and `server/Dockerfile`) and that tests pass (`npm test` and `npm run test:e2e` in `site/`, `cargo test` in `server/`) before reporting the work as done.
- For any UI or UX changes, use a Claude Artifact (via the `/prototype` skill) to get design confirmation before implementing — iterate on the artifact with the user until approved.
