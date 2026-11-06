# Changelog

## Unreleased

### Added

- Bulk Edit on Params and Headers: edit rows as `key:value` text, one per line
- Projects: switch between recent project folders from the top bar or the sidebar title

### Changed

- The code snippet panel looks like a code editor: coloured code, line numbers, and long lines scroll instead of wrapping
- Key and value tables are tidier: rows sit flush, text lines up with its column and is centred, turned-off rows fade
- The request name lines up with its folder path, and the URL bar text is centred in a bar as tall as Send
- Workspaces are now called projects in the app
- Team Sync is compact: one line for the branch, the commit box only when there is something to commit, and the
  remote, your identity and recent commits folded away
- Opening another folder now asks first if tabs have unsaved changes

## 1.0.0

First release under the Pigeon name.

### Added

- Requests, collections, folders, history and a collection runner
- Environments, collection variables and globals, with suggestions as you type
- JavaScript pre-request and post-response scripts with the `pm` API
- Point-and-click response checks
- Console showing every call
- Import from OpenAPI 3 JSON, v2.1 collection JSON and cURL, and collection export
- Team sync through git with pull, push and conflict resolution
- Packages for macOS, Windows and Linux
- A crash log, and carrying over settings from the earlier name

### Changed from the Dispatch prototype

- New name, icon and accent colour
- Split into `pigeon-core` and `pigeon-app`, with tests grouped by area
