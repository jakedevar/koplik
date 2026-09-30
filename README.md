# Koplik

**Measles outbreak intelligence, with a primary source behind every number.**

> Demonstration project; not medical or public-health advice; not affiliated with CDC or WHO.

Koplik spots are the earliest visible sign of measles. Koplik answers five questions for any US
county or country:
- what is happening;
- how fast it is spreading (R_t);
- where it is likely to spread next (a stochastic SEIR forecast);
- what happens if vaccination coverage changes (an in-browser WebAssembly simulator);
- why you should trust the answer (a content-addressed provenance trail back to the primary source).

**Status:** pre-alpha. It is being built live by an agent team run by the
RSI harness manager, using the same integrator model RSI uses to build itself.

| Read | For |
|------|-----|
| [`AGENTS.md`](AGENTS.md) | How work is done here: principles, hard rules, landing |
| [`thoughts/shared/project/koplik-spec.md`](thoughts/shared/project/koplik-spec.md) | Product spec: Epics, intent, acceptance criteria |
| [`thoughts/shared/manager/`](thoughts/shared/manager/) | Manager brief and worker contract |

Branches: `rolling` is agent intake (fast-forward only). `main` is operator-promoted from the QA-green SHA.
