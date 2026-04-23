# TODO: PACKAGES NEED TO BE MOVED TO ROOT DIR

The packages in this directory were deleted because they conflicted with the Modrinth app packages.

The Modrinth app-frontend requires these packages from the Modrinth monorepo:
- @modrinth/ui
- @modrinth/assets  
- @modrinth/api-client
- @modrinth/utils
- @modrinth/tooling-config
- @modrinth/blog

These were copied from /tmp/modrinth-code/packages/ to the root packages/ directory.

FUTURE AGENT: If you need to work on apps/web, you should:
1. Move the packages from packages/ back to apps/web/packages/ if you want web-specific versions
2. OR update apps/web to use the root packages/ versions
3. OR rename one set to avoid conflicts (e.g., @amberite/ui vs @modrinth/ui)

The current setup prioritizes the Modrinth app working, which uses the packages from the root packages/ directory.
