# spawn-at: rules for agents

spawn-at is a Rust command-line tool plus a GNOME Shell extension that places windows without flicker on GNOME Wayland.

Read this file fully, then read CLAUDE.md in full (build and test commands and more rules). If the two ever differ, follow the stricter one.

## Read first on every task

- `docs/KNOWLEDGE.md`: read only its table of contents, then the sections that match the task. Do not read the whole file unless the task needs it.
- `docs/evidence/INDEX.md`: the list of every run, one row per run.
- `docs-site/src/content/docs/experiments/index.md`: the experiments index, with the verdict of every run page.
- `docs/evidence/<run-name>/README.md` and `findings.md`: the evidence README and the story of the run you build on.

## Stop and report

Stop the task, write down what happened, and report to the owner before doing anything else on any of these:

- a gnome-shell crash or restart;
- signal 5, 6 or 11;
- "uncatchable exception";
- a Mutter window or focus assertion, such as `meta_window_*` or `focus_window`;
- any journal line that names spawn-at or our extension;
- the live Shell process id or start time changed.

## Never

- Touch the live GNOME session, the real extension or the real home folder.
- Run `spawn-at install`, `spawn-at uninstall` or `spawn-at update` for real.
- Install the extension or the binary unless the task says to.
- Use `pkill`, `killall` or any kill matched by a name or pattern. Signal only PIDs you recorded from a child you started.
- Run `rm` outside your scratch folder.
- Push `main`, merge, tag or force-push.
- Use `--tags`, `--all` or `--mirror`.
- Add `Co-Authored-By`, "Generated with" or session links to commits, PR text or any file.
- Edit `src/` or `assets/` in a measurement or documentation task.
- Edit `AGENTS.md` or `CLAUDE.md`, unless the task is about the rules (then change both together).
- Run real X11 or XWayland apps in tests. Test only with the fixture and our own scratch test clients, and never close a focused real XWayland window.
- Set `LD_PRELOAD` in the shell session, `systemctl --user set-environment`, `~/.profile` or any file. Use it only on the test client's own process.
- Push `archive/x11-experiments` or publish a release.

## How to work

- One job at a time. No parallel measurements.
- Read big files in parts, never over 400 lines whole.
- Paste real command output. Never retype it.
- Say "not verified" for anything you did not run, and list what could not be run.
- Make small conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`).
- Run fmt, clippy and tests after code changes.
- After each task in a long job: commit, push the branch by name, and write a checkpoint file in the run's evidence folder.
- Stop a task that runs over about 90 minutes, write what is done and not done, then move on.
- If something is unclear, take the safe choice and write down which one. If no safe choice is clear, stop and ask.

## Recording results (after every run)

This section is identical in CLAUDE.md; change both together.

Why: we must always be able to see what we tried, what we saw, and what we still do not know. A run is not finished until it is written down.

When: after every run that measures or tests something, big or small, and after anything that changes what we believe. Do it before you send your final report. Runs that found nothing, or failed, are written down too.

Where things go:
- Small raw data, logs and sample files: docs/evidence/<run-name>/data/
- The story of the run in plain English: docs/evidence/<run-name>/findings.md. README.md in the same folder says what the run was and how to read the files.
- Short facts worth keeping: docs/KNOWLEDGE.md, under the right heading. Dead ends go under "Dead ends".
- Anything a user will notice: CHANGELOG.md under Unreleased.
- The website page for the run: write the entry in docs/TEST_LOG.md. The experiments page 6.NN is written by hand from that entry, using only numbers that are in the log. Pages go live only through a docs pull request that the owner merges.
- One list of every run: docs/evidence/INDEX.md. One row per run: name, date, the question, the result in one line, the branch, and the website page or "not on the site yet". Add your row in the same commit as your findings.
- A mistake in something we wrote earlier: fix it where it appears, add a dated "Correction" line that says what was wrong and what is true, and list it under "Corrections" in your findings.md.

Be honest:
1. Say what you did, what you saw and how sure you are.
2. Label each claim. MEASURED: you ran it, and you name the file or cell. READ: from source code or docs, and you name the file or link. GUESS: your reasoning, not tested.
3. Give counts as "N of M runs", with the app, size, build and conditions.
4. Say what was not run, and why.
5. Never delete or quietly rewrite a wrong or failed result. Leave it, add a dated correction, and mark the old page "Superseded by ...".
6. If a new result disagrees with an old one, say so on both pages.
7. Do not say a fix works unless you measured before and after under the same conditions.

Write in plain English:
- Short sentences, one idea each.
- Use everyday words. Use a technical word only when there is no plain way to say it, and explain it in one sentence the first time it appears on a page.
- Use numbers, not adjectives: "17 of 20 runs", not "most runs".
- Say what it means for the person using spawn-at.
- Keep bold and code formatting light.
- How to shape a page for a run is in docs-site/src/content/docs/contributing/writing-docs.md, section "Recording a run".

Before you finish a run, copy this checklist into your final report and mark each line done, not done (with the reason), or not needed:
- [ ] findings.md and README.md written in docs/evidence/<run-name>/
- [ ] row added to docs/evidence/INDEX.md
- [ ] new facts and dead ends added to docs/KNOWLEDGE.md
- [ ] CHANGELOG.md updated if a user would notice
- [ ] entry added to docs/TEST_LOG.md
- [ ] corrections made where earlier statements were wrong
- [ ] "not run" list and "not sure" list written

## Final report

- The verdict first.
- Tables of numbers.
- What was measured, read and guessed.
- What was not run.
- Tool changes.
- Incidents.
- Decisions for the owner.
- The checklist from the shared section, each line marked done, not done (with the reason) or not needed.
