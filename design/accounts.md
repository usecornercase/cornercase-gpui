# Accounts

Code: `src/panels/accounts.rs`, `src/panels/accounts/`

- **What the sheet covers**: the accounts agents log in with (`cornercase account`) and profiles (`cornercase profile`). One section per agent that takes accounts: its own login and each account, whether it is signed in, the default one marked `●`, names, adding (and logging in, in a tab of that account) and removing. Profiles, a tool for scripts, come after.
- **The list first, the logins after**: `account list --check` asks each agent whether it is signed in and can take a while, so the sheet shows `account list` at once and then the checked list. A failed check leaves the list as it is (the logins only stay unknown). A check that answers while a change runs is dropped: the change is newer and lists again itself.
- **After every change, list again**: a change that prints its own `--json` answer is shown at once, then `account list` runs again to bring the logins. While a change runs, the buttons wait for it.
- **Logging in a new account**: `account add --login` opens the login tab in the workspace this window shows, and the sheet closes on that tab so the login can be done. "log in now" can be turned off per agent.
- **Names are checked before running**: `valid_name` mirrors what `account add` takes (letters, digits, `.`, `_` and `-`, at most 32, never `.` or `..`), so the sheet says why at once instead of showing cornercase's error. Every command puts names after `--`, so a name that starts with `-` is never read as an option.
- **Token variables win**: a token variable set in the server's environment wins over every account, so the sheet warns about it above the agents.
- **Profiles**: a profile maps each agent to an account; an agent left out uses its own login. While a profile is in use it is the default, and making an account the default leaves it; "No profile" is `profile use --system`.
- **One question at a time**: one rename field and one confirmation are open at most; opening one closes the other. Removing an account deletes its folder, so it asks first and says that the keychain item stays (logging out is the agent's own command, in a tab of that account).
- **Errors read as sentences**: `clean` drops `cornercase account:` and `error:` from a failed command's message before it is shown under the heading.
