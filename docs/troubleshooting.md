# Troubleshooting

## Sending requests

| What you see | What to do |
| --- | --- |
| Red "Unknown variable" | Pick an environment at the top right, or add the variable. Check any "did you mean" hint. |
| "The server name could not be found" | Check the URL, or that `{{baseUrl}}` holds a full address |
| "The server kept closing the connection" | Pigeon tried 3 times. Check http or https, the port, and any VPN or proxy. |
| "The server took too long" | Raise the request timeout in Settings |
| A pre-request script failed | The request wasn't sent. Open the Console for the error and its line number. |
| A post-response script failed | The response is fine. The banner above it shows the script's error. |

## Team Sync

| What you see | What to do |
| --- | --- |
| "Git is not installed" | Install [git](https://git-scm.com/downloads) and reopen Pigeon |
| "The repository refused your login" | Set up an SSH key or a saved token for your git service. Check it in a terminal with `git push`. |
| "Git needs your name and email" | Fill in **Your identity** in Team Sync |
| "Someone else pushed first" | Press **Pull**, then **Push** |
| "Merge in progress" | Choose **Keep mine** or **Keep theirs** for each file, then **Finish merge** and **Push** |
| "Saving is paused" | A file couldn't be read, usually a conflict. Resolve it in Team Sync. The Console's Issues tab lists the files. |
| "That folder is not empty" when cloning | Choose a new folder |

## Installing and opening

| What you see | What to do |
| --- | --- |
| macOS: "Pigeon can't be opened" or "cannot be verified" | The build isn't notarized. Right-click Pigeon, choose **Open**, then **Open**. If that's not offered, use **System Settings › Privacy & Security › Open Anyway**. |
| Windows: "Windows protected your PC" | The build isn't signed. Choose **More info**, then **Run anyway**. |
| Linux: `dpkg` reports missing packages | Run `sudo apt-get install -f` |
| Linux: won't start, mentions GLIBC | Pigeon needs Ubuntu 22.04, Debian 12 or newer |
| Linux: Import's file picker doesn't open | Install `xdg-desktop-portal` or `zenity`, or paste into the Import box |
| Window is blank or won't open | Update your graphics drivers. Virtual machines may need 3D acceleration turned on. |

## Reporting a problem

Include the version (Settings or `pigeon --version`), your system, and what you did. If Pigeon crashed, attach
`crash.log` from the settings folder listed in the [user manual](user-manual.md#where-your-data-lives).
The Console's copy button can capture the calls involved, but check them for tokens first.
