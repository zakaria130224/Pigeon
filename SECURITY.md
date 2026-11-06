# Security

## Reporting a problem

Report security problems privately to **[add a contact address before release]**, not in a public issue. Include
the version, your system and how to reproduce it.

## How Pigeon handles your data

- There is no account, and Pigeon sends no usage data. It only contacts the servers you send requests to and your
  git remote.
- **Workspace files aren't encrypted.** Anyone who can read the folder or the git repository can read tokens and
  passwords saved in requests and environments. Keep secrets in globals or variables set by scripts, and
  restrict who can access the repository.
- Settings, history and values set by scripts are stored unencrypted in your personal settings folder. History
  includes the requests you sent.
- The Console holds full requests and responses in memory until you close Pigeon. Check copied logs for secrets
  before sharing them.

## Scripts

Scripts run in a sandbox with no access to files or other programs, with limits on time and memory. They can read
your variables and send network requests. **Scripts pulled from a shared repository run when you press Send**,
so a repository's write access should be limited to people you trust.
