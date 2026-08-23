# Native Checkout Accessibility Acceptance

Automated coverage validates retained preorder focus traversal, Enter/Space activation, meaningful
labels, enabled/value/ready state, minimum 44×44 logical targets, and zero-duration reduced-motion
resolution across catalog, cart, payment, and receipt actions.

Before a milestone release, a human tester must also run the signed POS example on the documented
Wayland baseline and verify:

- visible focus is never clipped by either scroll pane;
- screen-reader announcement order matches the visual catalog/order layout;
- the trusted PIN and confirmation surfaces are distinguishable from plugin content;
- approved, declined, timeout, unavailable, receipt, and preview status are announced once;
- 200% text scaling does not obscure payment controls or the fixed order summary;
- reduced motion produces no sliding/fading movement or flashing.

Record the tester, date, compositor, scale factor, assistive technology, and any waiver in the
milestone validation report. These native visual/audio judgments cannot be established by a model
test alone.
