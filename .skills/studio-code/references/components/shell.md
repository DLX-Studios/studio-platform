# Application shell and navigation

These components structure the application shell. Declaring navigation metadata or item labels does not implement routing or tab state.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Scaffold

Provide the full application content shell.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Full-size vertical shell; its name does not mean this component scaffolds filesystem files.

```svelte
<Scaffold id="example-scaffold">
  <AppBar id="shell-appbar"><Text id="shell-title" text="My app" /></AppBar>
  <Column id="shell-body"><Text id="shell-copy" text="Workspace" /></Column>
</Scaffold>
```

## AppBar

Arrange top-level application controls in a bar.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native bar layout; child controls own their actions.

```svelte
<AppBar id="example-app-bar">
  <Text id="appbar-title" text="Workspace" />
  <Button id="appbar-save" label="Save" />
</AppBar>
```

## Sidebar

Arrange side navigation or supporting content.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native side panel layout; it does not load routes automatically.

```svelte
<Sidebar id="example-sidebar">
  <Column id="sidebar-content"><Button id="sidebar-home" label="Home" /></Column>
</Sidebar>
```

## NavigationBar

Arrange navigation controls horizontally.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Child actions must change app-owned navigation state.

```svelte
<NavigationBar id="example-navigation-bar">
  <Button id="navbar-home" label="Home" />
  <Button id="navbar-settings" label="Settings" />
</NavigationBar>
```

## NavigationRail

Arrange navigation controls vertically.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Child actions must change app-owned navigation state.

```svelte
<NavigationRail id="example-navigation-rail">
  <Button id="rail-home" label="Home" />
  <Button id="rail-settings" label="Settings" />
</NavigationRail>
```

## Tabs

Display tab labels and app-managed panel content.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Header labels render; automatic selection and content switching are not wired by items alone.

```svelte
<Tabs id="example-tabs" items={["Home", "Settings"]}>
  <Text id="tab-panel" text="Selected panel" />
</Tabs>
```

## Breadcrumb

Lay out a path of navigation items.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Displays authored children. Route/value metadata does not automatically navigate.

```svelte
<Breadcrumb id="example-breadcrumb">
  <Text id="crumb-home" text="Home" />
  <Text id="crumb-current" text="Settings" />
</Breadcrumb>
```

## Stepper

Display progress through a sequence of stages.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native display highlights the zero-based current step; advancing it is app-owned.

```svelte
<Stepper id="example-stepper" items={["Details", "Review", "Done"]} step={1} />
```

## Pagination

Display a current-page indicator.

**Props:** [Navigation/data contract](contracts.md#navigation-and-data-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Current renderer formats the page indicator; it does not fetch data or wire previous/next actions.

```svelte
<Pagination id="example-pagination" page={1} pages={10} />
```

## MenuBar

Lay out a top-level menu strip.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native horizontal strip; child buttons own actions, not an OS-native application menu integration.

```svelte
<MenuBar id="example-menu-bar">
  <Button id="menu-file" label="File" />
</MenuBar>
```

## StatusBar

Lay out compact status content.

**Props:** [Extended contract](contracts.md#extended-profile).

**Children:** any. **Events/state:** No guest input event is wired for this example.

**Current support:** Native status strip; put visible content in child Text nodes.

```svelte
<StatusBar id="example-status-bar">
  <Text id="status-ready" text="Ready" />
</StatusBar>
```
