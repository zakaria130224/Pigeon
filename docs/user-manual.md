# Pigeon user manual

Pigeon lets you send requests to web APIs, keep them organised, test the answers, and share all of it with
your team through git.

On macOS the shortcuts below use **⌘**. On Windows and Linux use **Ctrl** instead.

## The window

| Area | What it does |
| --- | --- |
| Top bar | Projects, Import, search, current branch, **Pull**, **Push**, light/dark, Settings |
| Left rail | Switches the sidebar: Collections, Environments, History, Team Sync |
| Sidebar | The open project's name, which switches projects, then your collections, environments, history or sync status |
| Tabs | One tab per open request, with the active environment picker on the right |
| Request and response | The middle of the window: edit above, see the answer below |
| Bottom bar | Console button, git branch and pending changes, active environment |

## First steps

The first time Pigeon opens, it creates a project folder called **Pigeon Workspace** in your Documents
folder, with a sample collection.

1. Click **List users** under **Sample API › Users**.
2. Click **Send**.
3. The answer appears below, with its status, time and size.

## Requests

Choose a method (GET, POST and so on), type a URL and press **Send** or **⌘↵**. You can also paste a whole
`curl` command into the URL box and it fills in the request for you.

| Tab | What goes there |
| --- | --- |
| Params | Query parameters. They stay in step with the URL. Untick a row to leave it out. |
| Authorization | Inherit from the folder or collection, No Auth, Bearer Token, Basic Auth or API Key |
| Headers | Your own headers. Content-Type and Authorization are added for you from the other tabs. |
| Body | none, form-data (text fields only), x-www-form-urlencoded, or raw with JSON, Text, XML, HTML or JavaScript. **Beautify** tidies JSON. |
| Scripts | JavaScript that runs before sending and after the response. See [Scripts](#scripts). |
| Tests | Point-and-click checks such as "Status code is 200" |
| Docs | Notes about this request |

On Params and Headers, **Bulk Edit** swaps the table for plain text, handy for pasting a block of headers. Write one
`key:value` per line and start a line with `//` to leave it out. Changes apply as you type, and **Key-Value Edit**
takes you back to the table. Descriptions aren't shown in the text, but a row keeps its description as long as its key
stays the same.

Press **Save** or **⌘S** to save. An unsaved request shows a dot on its tab. The **</>** button shows the
request as cURL, JavaScript or Python, ready to copy.

| Shortcut | Action |
| --- | --- |
| ⌘↵ | Send |
| ⌘S | Save |
| ⌘T | New tab |
| ⌘W | Close tab |
| ⌥⌘C (Ctrl+Alt+C) | Open or close the Console |

### Reading the response

- **Body** shows the answer. **Pretty** formats JSON and **Raw** shows it exactly as received.
- **Headers** lists the response headers.
- **Test Results** shows every check and `pm.test` with PASS or FAIL.

If a connection is dropped partway, Pigeon tries again up to 3 times before showing an error. This also
applies to POST and other requests that change data, so a server could receive such a request twice.

## Collections and folders

Use **New › Collection**, then right-click a collection or folder for everything else: add a request or folder,
rename, duplicate, move up or down, export and delete.

Double-click a collection, or choose **Open overview**, to edit its description, authorization, variables and
scripts. For a folder, choose **Edit folder**. Requests set to "Inherit" use the authorization of the nearest
folder or collection above them.

The search box in the top bar filters collections and history by name or URL.

### Running a whole collection

Right-click a collection and choose **Run collection**. Pick how many iterations to run and a delay, then
press **Run collection**. Requests run in order, and variables set by scripts carry on to the next request.
**Stop** ends the run.

## Variables

Write `{{name}}` in a URL, header, body or sign-in field. Known variables turn green and unknown ones turn red,
with a "did you mean" hint under the URL when a name looks mistyped.

| Kind | Where to edit it | Shared with the team? |
| --- | --- | --- |
| Environment | Environments in the left rail. Pick the active one at the top right. | Yes |
| Collection | Collection overview, Variables tab | Yes |
| Global | Environments, then Globals | No, this computer only |

If a name exists in more than one place, the environment wins, then the collection, then globals.

**Fresh values** are filled in for each send: `{{$guid}}`, `{{$randomUUID}}`, `{{$timestamp}}`,
`{{$isoTimestamp}}`, `{{$randomInt}}` and `{{$randomBoolean}}`.

### Suggestions

Type `{{` and a list opens with your variables, their current values and a badge: **E** environment,
**C** collection, **G** global, **$** fresh value. Keep typing to narrow it. Use the arrow keys, then **Enter** or
**Tab** to insert, or click a row. **Esc** closes the list. Hover over any `{{variable}}` to see its value and
where it comes from.

## Scripts

Requests, folders and collections each have a **Pre-request** and a **Post-response** script, written in
JavaScript. For each send, they run from the outside in: collection, folders, then the request.

```js
pm.test("Returns a list of users", () => {
  pm.response.to.have.status(200);
  const users = pm.response.json();
  pm.expect(users).to.be.an("array");
  pm.environment.set("userId", users[0].id);
});
```

The **Snippets** list beside the editor inserts common examples. `console.log` output appears in the Console.
If a pre-request script fails, the request is not sent. A script is stopped after 30 seconds.

**Values set by scripts stay on this computer.** When a script sets an environment or collection variable, the
value is kept locally and used in place of the shared one, so a login token can't be pushed to your team by
accident. You can see these under **Set by scripts** on the environment or collection page, and **Clear these**
removes them.

Everything available to scripts is in the [scripting reference](scripting-reference.md).

## Console

Click **Console** in the bottom bar to see every call, including those from the runner and scripts. Click a call
to see its request and response headers and bodies. You can filter, search, copy all logs and clear them. Git
actions and script output appear here too, and the **Issues** tab lists problems reading the project. A red
number on the button counts errors. The Console is cleared when you close Pigeon.

## Importing and exporting

Click **Import**, then choose files or paste text.

| You have | What happens |
| --- | --- |
| OpenAPI 3 file in JSON | Becomes a collection. Tags become folders, the server becomes `baseUrl`, and parameters, bodies and sign-in settings carry over. |
| Collection export (v2.1 JSON) | Becomes a collection, including its scripts |
| Environment export | Becomes an environment |
| A `curl` command | Opens as a new request |

After an OpenAPI import, optional parameters are switched off, and tokens and keys are blank variables for you to
fill in under the collection's Variables. If the file has no full server address, `baseUrl` starts as
`http://localhost`.

To export, right-click a collection or environment and choose **Export**.

## Projects

A project is a folder holding collections and environments, usually a git repository you share with your team.
Open **Projects** in the top bar, or click the project's name at the top of the sidebar, to see the projects you
opened most recently.

- Click a project to open it. This closes your tabs, so Pigeon asks first if any have unsaved changes.
- Hover over a project and click the **×** to take it off the list. The folder stays where it is.
- **Open project folder…** adds a folder from your computer, and **Clone a team repository…** adds one from your
  team.
- **Reload from disk** reads the open project's files again.

If a project's folder has moved, open it from its new place and remove the old entry.

## Team Sync

Your collections and environments are ordinary files in the project folder. Team Sync shares them through a
git repository. You need [git](https://git-scm.com/downloads) installed.

### Joining a team

Open **Projects**, choose **Clone a team repository…**, then paste the repository address.

### Starting a shared project

1. Create an empty repository on your git service.
2. Open **Team Sync**, paste its address, and choose **Set up git in this folder**.
3. Under **Repository**, enter your name and email and choose **Save identity**.
4. Press **Push**.

### Daily use

| Button | What it does |
| --- | --- |
| **Pull** | Saves your changes as a commit, then gets your team's changes |
| **Push** | Saves your changes as a commit, then sends them to the team. If someone pushed first, pull and push again. |
| **Sync** | Pull, then push |
| **Commit only** | In the **…** menu. Saves a commit on this computer without sending it |

The **Changes** list shows what is different. Click a file to see exactly what changed. The arrows beside the
branch count the commits waiting to be pushed and pulled. By default Pigeon also pulls when it starts, and you
can turn that off in Settings. You can switch or create branches from the branch list.

**Repository** holds the remote address and your name and email, and **Recent commits** lists the latest work.
Click either heading to open it. Hover over a commit to see who made it and when.

### When two people change the same request

Pigeon shows **Merge in progress** with the files involved. For each one, choose **Keep mine** or **Keep theirs**,
then **Finish merge** and **Push**. **Abort merge** puts everything back. Saving is paused until this is resolved,
so nothing is overwritten.

Each request is its own small file, so two people working on different requests don't clash.

### Signing in to your repository

Pigeon uses your computer's git login and can't ask for a password itself. Set up an SSH key or a saved token
for your git service first. If a push is refused, see [Troubleshooting](troubleshooting.md).

### What gets shared

- **Shared:** collections, folders, requests, their scripts, environments and collection variables, including
  any token or password you typed into them. Use a variable set by a script, or a global, for secrets.
- **Not shared:** settings, history, globals and values set by scripts.
- Scripts your teammates push run on your computer when you press Send. They can't reach your files, but they
  can read your variables and send requests, so only share a project with people you trust.

## Settings

Click the gear in the top bar for the theme, following redirects, pulling at start-up, the request timeout, and
the project folder. The Pigeon version is shown at the bottom.

## Where your data lives

| What | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Project | `~/Documents/Pigeon Workspace` | `Documents\Pigeon Workspace` | `~/Documents/Pigeon Workspace` |
| Settings, history, crash log | `~/Library/Application Support/Pigeon` | `%APPDATA%\Pigeon` | `~/.local/share/Pigeon` |

Uninstalling Pigeon removes the program only. These folders stay until you delete them.

If you used this app under its earlier name, Dispatch, Pigeon copies your settings and history the first time it
opens and keeps using the same project folder.

## What Pigeon doesn't do yet

- Uploading files in form-data bodies
- Libraries in scripts through `require` (such as lodash, crypto-js or moment) and `pm.visualizer`
- OAuth sign-in flows (paste a token you got elsewhere instead)
- Keeping cookies between requests
- OpenAPI files in YAML, and Swagger 2 files
- WebSocket, GraphQL and gRPC requests
- Drag and drop in the collection tree (use Move up and Move down)
- Automatic updates
