# Contributing to Catalyst Quantum MCM Runtime Engine

We welcome community contributions to help improve the performance, reliability, and security of this runtime.

---

## 🛠️ Contribution Workflow

1. **Fork and Clone:** Fork the repository on GitHub and clone your fork locally.
2. **Setup Development Target:** Configure your local environment according to the [README](README.md) instructions.
3. **Create a Feature Branch:** 
   ```bash
   git checkout -b feat/your-feature-name
   ```
4. **Write Tests:** Ensure you add unit tests in Rust to cover your code changes.
4b. **Memory Safety (for FFI changes):** If your change touches `ffi.rs`, `capi.h`,
    or the callback mechanism, run the memory safety suite:
    ```bash
    tests/memory_safety/run_valgrind.sh
    tests/memory_safety/run_asan.sh
    ```
5. **Verify Local Setup:** For Python integration validation, locally bootstrap the `frontend_python/` directory as detailed in the [README](README.md) and execute tests:
   ```bash
   python -m pytest
   ```
6. **Code Quality:** Format and lint Rust code before committing:
   ```bash
   cargo fmt
   cargo clippy
   ```
6b. **Documentation:** Verify Rust documentation compiles cleanly:
    ```bash
    cargo doc --no-deps 2>&1 | grep -i warning
    ```
7. **Commit Guidelines:** Follow the Conventional Commits specification (see below).
8. **Submit PR:** Open a Pull Request against our `main` branch.

---

## 📌 Git Commit Message Guidelines

Commits must use the Conventional Commits specification, written in the present tense and imperative mood:

```
<type>(<optional scope>): <description>
```

### Common Types:
* `feat`: A new feature or API capability.
* `fix`: A bug fix.
* `docs`: Documentation-only changes.
* `style`: White-space, formatting, semicolons (no code logic changes).
* `refactor`: Structural refactoring that neither fixes a bug nor adds a feature.
* `perf`: Changes aimed at improving execution speed.
* `test`: Adding or correcting tests.
* `chore`: Auxiliary build configurations or dependency updates.

### Example:
`feat(qubit): add capacity exhaust validation limits`
