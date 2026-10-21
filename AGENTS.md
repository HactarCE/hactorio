## jj workflow

Use `jj`, never `git`.

Before starting a unit of work, run `jj status`.

- If the description is unrelated to current work, run `jj new`.
- If the description is related to current work, continue on the current commit.

After completing a unit of work, run `jj describe` and describe the whole set of changes, including the existing work on the commit (if any) and the new work you just did.
