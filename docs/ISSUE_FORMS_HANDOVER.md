# GitHub issue forms handover

Implemented 2026-11-19.

## Purpose and behavior

New issue reporters choose between a bug report and a feature request. Bug reports collect enough environment information for triage while keeping specialized details optional. Feature requests focus on the user's workflow and desired outcome.

## Files

| File | Purpose |
| --- | --- |
| `.github/ISSUE_TEMPLATE/bug-report.yml` | Required app version/edition, OS, OS version, hardware, actual/expected behavior, and reproduction steps. Optional relevant setup and attachments. |
| `.github/ISSUE_TEMPLATE/feature-request.yml` | Required problem/workflow and desired outcome, with optional alternatives/context. |
| `.github/ISSUE_TEMPLATE/config.yml` | Removes the blank-issue option from GitHub's template chooser. |
| `docs/ISSUE_FORMS_HANDOVER.md` | Implementation, validation, rollout, and maintenance notes for the next coding agent. |

## Design decisions

- Exact app and OS versions use free text, avoiding release-by-release maintenance of version dropdowns.
- OS is a dropdown with macOS, Windows, Linux, iOS, and Other / unknown. There is no preselected answer.
- Version and edition share one field. The desktop hint matches the existing bottom-left status display and `src/routes/help/+page.svelte` guidance. The iPhone app doesn't show its version, so the iPhone hint points to Settings ▸ General ▸ iPhone Storage ▸ Rupert. (iPhone build numbers are timestamps and not useful to reporters.)
- Hardware is one field rather than separate mandatory CPU/GPU/RAM questions. Reporters may enter "unknown" for details they cannot find.
- Expected and actual behavior share a field; reproduction has its own field and accepts an explanation of intermittent failures.
- Folder providers, scanner details, AI configuration, and OCR versions are optional because their relevance depends on the bug.
- Screenshots/logs are optional and use a normal textarea so reporters can paste text or attach files. Prompts ask users to remove private information.
- There are no mandatory acknowledgment checkboxes, automatic comments, labels, assignees, or project assignments. No existing repository label or project configuration is assumed.

## Validation and rollout

Local checks parse the YAML, check supported field structures and unique IDs, verify the intended required fields and OS choices, and check whitespace with `git diff --check`. Application tests/builds are unnecessary for this configuration-only change.

The forms become available after these files are committed and merged into the repository's default branch. This change does not itself commit, push, or publish them. Local validation cannot confirm GitHub's rendered UI or server-side acceptance.

After merging, verify on GitHub:

1. Open **Issues → New issue** and confirm the two choices appear without a blank-issue option.
2. Open the bug form and confirm all prompts, hints, and OS choices render correctly.
3. Try submitting with a required field empty and confirm GitHub blocks submission.
4. Confirm optional fields can remain empty and that the feature form requires only its two core fields. Avoid submitting a test issue unless wanted.

Required fields enforce completion in the form, not the truth or quality of an answer. Removing the blank option is a chooser setting, not a repository-wide schema: API/CLI-created issues and later edits should not be assumed to obey the forms. No bot or workflow audits those paths.

## Maintenance and possible follow-up

Edit the YAML files to change prompts or requirements. Keep IDs stable if app links later prefill fields. Update OS options if platform support changes. Do not add a version dropdown unless someone will maintain it for every release.

An optional future improvement is a **Copy system information** button in Rupert that collects version/edition, OS version, architecture, and basic hardware details. It is not implemented here. If added, update the bug form to accept the copied block and consider a Report a bug link that opens a prefilled form. Review collected information for privacy and support both desktop editions and iPhone as applicable.

References:

- [GitHub issue template and chooser configuration](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/configuring-issue-templates-for-your-repository)
- [GitHub form schema](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-githubs-form-schema)
