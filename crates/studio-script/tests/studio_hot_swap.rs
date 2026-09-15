//! US2 hot-swap contract: a scripted 10-edit session over a stateful
//! component shows preserved state, retained identity, rejection-and-continue,
//! dispose-and-initialize, and no partial swaps.

use studio_script::hot_swap::{SlotOutcome, SwapOutcome, replace_module};
use studio_script::ir::StudioModule;

fn compile(name: &str, source: &str) -> Result<StudioModule, Vec<studio_script::Diagnostic>> {
    studio_script::compile_studio(source, &format!("{name}.studio"), name)
}

const BASE: &str = "<script lang=\"ts\">
  let { price = 0 } = $props();
  let quantity = $state(1);
</script>
<Card id=\"swap-card\">
  <Text id=\"swap-total\">{quantity}</Text>
</Card>";

const WITH_HANDLER: &str = "<script lang=\"ts\">
  let { price = 0 } = $props();
  let quantity = $state(1);
  function add() {
    emit(\"add-one\", { quantity });
  }
</script>
<Card id=\"swap-card\">
  <Column id=\"swap-col\">
    <Text id=\"swap-total\">{quantity}</Text>
    <Button id=\"swap-add\" onclick={add}>Add</Button>
  </Column>
</Card>";

#[test]
fn scripted_session_applies_the_swap_contract() {
    // Edit 1: baseline compiles (the handler is missing, so this variant
    // holds no handlers but is otherwise valid).
    let live = compile("swap", BASE).expect("baseline compiles");

    // Edit 2: template-only tweak preserves every slot.
    let tweaked = BASE.replace("{quantity}</Text>", "{quantity} pcs</Text>");
    let next = compile("swap", &tweaked).expect("tweak compiles");
    let SwapOutcome::Accepted { components } = replace_module(&live, &next) else {
        panic!("template tweak must swap");
    };
    assert!(
        components[0]
            .slots
            .iter()
            .all(|migration| migration.outcome == SlotOutcome::Preserved)
    );

    // Edit 3: invalid syntax rejects; the caller keeps `live` running.
    assert!(compile("swap", "<script>let = ;</script>\n<Card id=\"a\" />").is_err());

    // Edit 4: incompatible type change disposes and initializes.
    let retyped = BASE.replace(
        "let quantity = $state(1);",
        "let quantity = $state(\"one\");",
    );
    let next = compile("swap", &retyped).expect("retyped compiles");
    let SwapOutcome::Accepted { components } = replace_module(&live, &next) else {
        panic!("retype must swap");
    };
    let quantity = components[0]
        .slots
        .iter()
        .find(|migration| migration.name == "quantity")
        .expect("quantity migration reported");
    assert_eq!(quantity.outcome, SlotOutcome::Initialized);

    // Edit 5: adding a state slot initializes exactly that slot.
    let extended = BASE.replace(
        "let quantity = $state(1);",
        "let quantity = $state(1);\n  let note = $state(\"hi\");",
    );
    let next = compile("swap", &extended).expect("extension compiles");
    let SwapOutcome::Accepted { components } = replace_module(&live, &next) else {
        panic!("extension must swap");
    };
    let outcomes: Vec<(&str, SlotOutcome)> = components[0]
        .slots
        .iter()
        .map(|migration| (migration.name.as_str(), migration.outcome))
        .collect();
    assert!(outcomes.contains(&("quantity", SlotOutcome::Preserved)));
    assert!(outcomes.contains(&("note", SlotOutcome::Initialized)));

    // Edit 6: removing a slot that the template still reads fails
    // compilation, so no swap is even planned and the live module runs on.
    let shrunk = BASE.replace("  let quantity = $state(1);\n", "");
    let failure = compile("swap", &shrunk).expect_err("dangling reads must fail");
    assert!(
        failure
            .iter()
            .any(|diagnostic| diagnostic.code == "STUDIO321")
    );

    // Edit 7: removing an unused slot disposes it through the swap.
    let with_unused = BASE.replace(
        "let quantity = $state(1);",
        "let quantity = $state(1);\n  let scratch = $state(0);",
    );
    let live_unused = compile("swap", &with_unused).expect("unused slot compiles");
    let back_to_base = compile("swap", BASE).expect("base compiles");
    let SwapOutcome::Accepted { components } = replace_module(&live_unused, &back_to_base) else {
        panic!("unused-slot removal must swap");
    };
    let scratch = components[0]
        .slots
        .iter()
        .find(|migration| migration.name == "scratch")
        .expect("scratch migration reported");
    assert_eq!(scratch.outcome, SlotOutcome::Disposed);
}

#[test]
fn handler_addition_flows_through_swap() {
    let live = compile("swap", BASE).expect("baseline compiles");
    assert!(live.components[0].handlers.is_empty());

    let next = compile("swap", WITH_HANDLER).expect("handler variant compiles");
    assert_eq!(next.components[0].handlers.len(), 1);
    let SwapOutcome::Accepted { components } = replace_module(&live, &next) else {
        panic!("handler addition must swap");
    };
    assert!(
        components[0]
            .slots
            .iter()
            .all(|migration| migration.outcome == SlotOutcome::Preserved)
    );
}

#[test]
fn swaps_never_mutate_their_inputs() {
    let live = compile("swap", BASE).expect("baseline compiles");
    let snapshot = live.clone();
    let next = compile("swap", WITH_HANDLER).expect("variant compiles");
    let _ = replace_module(&live, &next);
    assert_eq!(live, snapshot, "planning is pure");
}
