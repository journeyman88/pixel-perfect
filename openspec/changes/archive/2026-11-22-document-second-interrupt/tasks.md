## 1. Spec

- [x] 1.1 Rewrite *Ending watch* in the `watch-command` delta: the first interrupt finishes the render and exits with 0; a second exits at once with 130; what the output can look like then; how to recover. Add the scenarios.
- [x] 1.2 Check that the spec matches the code: the `ctrlc` handler in `src/main.rs` sends `Signal::Stop` on the first call and runs `std::process::exit(130)` on later ones. The README is already correct and stays unchanged.
