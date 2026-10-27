# mitcad-model-testkit

The mock geometry kernel of the MitCad model's tests (`MockKernel`,
`MockShape`; mitcad#777). Shapes record the operations that built them and
carry face names and face-pair edges the way the OCCT facade names them,
so references, recompute and the body state are tested without OCCT.

The crate depends on `mitcad-model-base` alone (the `Kernel` trait and
its data), never on `mitcad-model`: the model uses it in its own unit
tests as a dev-dependency, and a dependency back on the model would make
two copies of it. The model re-exports the crate as
`mitcad_model::testing` under its `testing` feature, so the test crates,
the import and the readers name the mock kernel as
`mitcad_model::testing::MockKernel`. The fixtures that need the model's
`Document` stay in the model (`mitcad_model::test_support`).

An edit here rebuilds only the test binaries, not the model. The crate has
no tests of its own; its ctest `core.model_testkit` checks that it builds.
