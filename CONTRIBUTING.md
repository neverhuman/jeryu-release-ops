# Contributing

Thanks for your interest in jeryu-release-ops. Outside pull requests are
welcome on GitHub.

## Where changes are reviewed

The GitHub repository is a read-only mirror. Review and merge happen on the
Jeryu forge: a pull request opened on GitHub is imported there, gated,
reviewed and merged, and the mirror push brings the result back. When that
happens the GitHub pull request is closed with a link to the forge change.

The importer is not live yet. Until it is, pull requests opened on GitHub are
read but not merged yet, and nothing can be merged on GitHub itself. Issues
and pull requests are still the right place to propose a change — expect a
reply, not a merge.

## What a change must hold to

- Tests and fixtures use invented data only. No real hosts, organisations,
  accounts or paths; use names such as `acme`, `globex` and `*.example`.
- Keep the commit subject a release note: it says what changed for a user.
- Do not edit `CHANGELOG.md`. Release notes are generated at release time.

## Running the gate locally

Run the repository's checks before you open a pull request:

```sh
just fast    # cargo check
just check   # the repository's own gate: builds, scripts and schema checks
```

The full required gate (`just required`) additionally needs the family's
governed tooling, so it is normally the forge that runs it on an imported
pull request. A change that passes `just check` is ready to propose.

There is no contributor licence agreement or developer certificate of origin
requirement at this time.
