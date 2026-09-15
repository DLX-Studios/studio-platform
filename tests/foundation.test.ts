import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "..");

describe("Phase 0 foundation", () => {
  test.each([
    "IMPLEMENTATION_PLAN.md",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "bun.lock",
    "THIRD_PARTY_NOTICES.md",
    "docs/security/THREAT_MODEL.md",
    "docs/upstream/oxide-audit.md",
    ".specify/integration.json",
    ".specify/memory/constitution.md",
    ".agents/skills/speckit-specify/SKILL.md",
    ".agents/skills/speckit-converge/SKILL.md",
    "docs/COMPONENT_CATALOG_PLAN.md",
    "docs/GPUI_COMPONENT_CAPABILITY_MATRIX.md",
    "specs/002-component-platform/spec.md",
    "specs/002-component-platform/plan.md",
    "specs/002-component-platform/tasks.md",
    "protocol/README.md",
    "protocol/fixtures/README.md",
    "sdk/assemblyscript/assembly/generated/README.md",
    "examples/pos-desktop/assets/README.md",
  ])("includes %s", async (relativePath) => {
    expect(await Bun.file(join(root, relativePath)).exists()).toBe(true);
  });

  test("records every approved upstream revision", async () => {
    const research = await Bun.file(
      join(root, "docs/upstream/README.md"),
    ).text();

    expect(research).toContain("29cd89882465d6ebfe00af2ada6f89951581c580");
    expect(research).toContain("e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b");
    expect(research).toContain("e158684b23d9cb043fed3989ca252212046dabca");
    expect(research).toContain("fecccf8c0d641efc75152fa206bbb941fa990c70");
    expect(research).toContain("b8b4228d9a1cb2bb108432241bcb5d8e6784a035");
  });

  test("pins the official Codex Spec Kit integration", async () => {
    const integration = await Bun.file(
      join(root, ".specify/integration.json"),
    ).json();

    expect(integration.version).toBe("0.15.2");
    expect(integration.default_integration).toBe("codex");
    expect(integration.integration_settings.codex.parsed_options.skills).toBe(
      true,
    );
  });

  test("keeps locked CI and Wayland build documentation aligned", async () => {
    const workflow = await Bun.file(join(root, ".github/workflows/ci.yml")).text();
    const building = await Bun.file(
      join(root, "docs/development/BUILDING.md"),
    ).text();

    for (const command of [
      "cargo clippy --locked --workspace --all-targets -- -D warnings",
      "cargo test --locked --workspace",
      "bun run check",
      "bun test",
      "./scripts/check-no-x11-features.sh",
      "cargo build --locked --release -p studio-app",
      "./scripts/check-no-x11.sh target/release/studio-app",
      "./scripts/test-headless-wayland.sh",
    ]) {
      expect(workflow).toContain(command);
      expect(building).toContain(command);
    }

    expect(workflow).toContain(
      "git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated",
    );
    expect(building).toContain("libwayland-dev");
  });

  test("tracks the unified component platform feature artifacts", async () => {
    const tasks = await Bun.file(
      join(root, "specs/002-component-platform/tasks.md"),
    ).text();
    expect(tasks).toContain("Unified Native Component Platform");
    expect(tasks).toContain("T003");
    expect(tasks).toContain("T036");
  });

  test("documents the component catalog plan and generated-artifact ownership", async () => {
    const catalog = await Bun.file(
      join(root, "docs/COMPONENT_CATALOG_PLAN.md"),
    ).text();
    expect(catalog).toContain("002-component-platform");
    expect(catalog).toContain("crates/studio-protocol");
    expect(catalog).toContain("ui.rs");
    expect(catalog).toContain("schemas");
    expect(catalog).toContain("generate_schema");
    expect(catalog).toContain("generate:protocol");
    expect(catalog).toContain("specs/002-component-platform");

    const quickstart = await Bun.file(
      join(root, "specs/002-component-platform/quickstart.md"),
    ).text();
    expect(quickstart).toContain("component_catalog_v1");
    expect(quickstart).toContain("generate:protocol");
  });

  test("documents component catalog validation in the locked build guide", async () => {
    const building = await Bun.file(
      join(root, "docs/development/BUILDING.md"),
    ).text();
    expect(building).toContain(
      "cargo test --locked -p studio-protocol --test component_catalog_v1",
    );
    expect(building).toContain(
      "cargo test --locked -p studio-components --test component_catalog",
    );
    expect(building).toContain("bun run generate:protocol");
    expect(building).toContain(
      "git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated",
    );
    expect(building).toContain(
      "bun test sdk/assemblyscript/tests/component-catalog.test.ts",
    );
  });
});
