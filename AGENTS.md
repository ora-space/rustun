## AGENTS.md

1. **Code Documentation**: Unless it is a standard, self-explanatory method (e.g., `new()`), every function must include a comment above the signature describing its purpose. Provide inline comments for any complex logic, non-trivial algorithms, or specialized branching within function bodies. Write comments in English.
2. **Explain "Why", not "What"**: Use comments to explain design rationale, business logic constraints, or non-obvious trade-offs. Code structure and naming should inherently describe the "what."
3. **Design for Testability (DfT)**: Favor Dependency Injection and decoupled components. Define interfaces via Traits to allow easy mocking, and prefer small, pure functions that can be unit-tested in isolation.
4. **Prefer Static Dispatch**: Use Generics and Trait Bounds over Trait Objects (e.g., `Box<dyn Trait>`) to leverage monomorphization and compiler optimizations, unless runtime polymorphism is strictly necessary.
5. **Make Illegal States Unrepresentable**: Use Enums with associated data to model state machines, rather than Structs with many optional fields.
6. **No Backward Compatibility**: Prioritize clean design over legacy support. Do **not** preserve compatibility layers "just in case." Break old patterns, remove deprecated code—adapt old to new, never vice versa.
