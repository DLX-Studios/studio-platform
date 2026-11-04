# Data model

Reuse RoutingManifest, TimerManifest, ProjectRouter, TimerSchedule and protocol GuestMessage/NavigationCommand. WebHost stores one router and one active timer schedule. Browser configuration contains schemaVersion1, routing, timers, asset allowlist and public title. Browser DOM retains nodes by ID. Worker maintains the committed root/registry and selected route; only host-selected page roots can mount. Requests carry bounded numeric IDs for watchdog and serialized delivery.
