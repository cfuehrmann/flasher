## Issues

Look at this repo. It is my flashcard application. I use it for myself. It has
multiple issues I need to fix.

### Issue 1: Difficult development experience and deployment

There is a .NET backend and a vite frontend. There is no single command to start
everything. And no single stack to update all dependencies. Two tech stacks are
needed on the dev machine, and .NET is needed on the host. Due to different tech
stacks in frontend and backend, there is no automatic type safety during usage
of the Web API. Overall, a dev experience that's mediocre at best.

### Issue 2: Problematic data storage

There is no professional database. JSON "data" need to be read on every start
and be cached. We should move to a less amateurish storage. It could be
Postgres. But preferably something lighter, like SqlLite. I'm open for
suggestions. There should be a goog migration experience though.

### Issue 3: Dated UX

### Issue 4: Dependency churn

I'm less certain about this one. A fact is that GitHub vulnerabilities show up
quite often for this repo, mostly in the (transitive) node dependencies. The
dependency tree should only be as big as necessary. But maybe more importantly:
There should be comparatively little churn and rare vulnerabilities.

This is related to the future tech stack, see below.

### Issue 5: Authentication

I want to move to a modern passkey experience, where users can register with a
passkey, and then add additional passkeys, name them, delete them, and so on.

That said, I may ultimately want a "central" passkey service on my domain, where
other apps are "clients". Not sure right now where to put this: At least
temporarily in this app, or defer, or elsewhere.

## Aspects

### Which tech stack?

Bun? Vite? Or maybe Rust? Rust may look weird, and initially other AIs tried to
dissuade me from that. But recently I migrated another repo from Bun to Rust,
and I'm glad I did. See /home/carsten/omega/dev.

So I would quite like Rust, unless there are strong arguments against it

### Testing

I'd like tests to be as e2e as possible. Ideally, testing real browser use
cases.

A special thing is that I love periodical mutation testing, see the
aforementioned omega project. I'm aware mutation testing also exists for .NET
(Stryker) and JS/TS (also Stryker?). But I think the Rust experience might be
the best w.r.t. mutation testing.

I also like snapshot testing (see Omega project or flasher project). Because it
means writing fewer assertions manually and getting nicely readable behavior
descriptions. Of course, how well that really works strongly depends on what's
being tested.

### Computer vision

I'd like things to be set up in such a way that you can check the app with
computer vision, before getting back to me, when you are working on UX.

### Token cost at scale

The project should not needlessly burn tokens due to bad architecting or lacking
quality gates. Quality gates "only" cost time. Needless bugfixes burn tokens.
And this laptop is very, very fast.

### Iteration speed

One might think that the iteration speed is slow for Rust. But due to my
experience with Omega, I know it's not so bad, at least when the builds are
incremental. And tests in Rust tend to run very fast. mutation testing in
particular (considering that mutation testing is always slow.)

## Plan needed first

What I first need is a plan. That plan might change as we go along. Because the
plan might be rather big, and I need to read it as a human, I'd like it as a
nice-UX HTML that we keep up to date.

My spec here should be copied to an extra md file, so it persists. I also put
the spec in /home/carsten/tmp.md.

### Migration strategy

Maybe we should make a parallel git repo. Not sure though. What we should
certainly do is keep the old code fully intact while the new one grows.

### Lighthouse (added 2026-07-27)

Lighthouse scores matter. Optimize them within reason — and do an early pass,
before bad habits (no loading state, missing meta, unoptimized asset delivery)
cement themselves. Not chasing 100s at all costs; no heroics.
