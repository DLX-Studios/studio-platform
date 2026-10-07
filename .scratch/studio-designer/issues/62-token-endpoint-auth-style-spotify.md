# 62 [R]: Descriptor-declared token-endpoint authentication and Spotify

**What to build:** Every maintained provider authenticates to the token endpoint by placing `client_secret` in the request body. Spotify authenticates with HTTP `Authorization: Basic base64(client_id:client_secret)` and rejects body secrets, which is why it has no descriptor: a `spotify` descriptor would parse, validate, and then fail at the exchange. Add a descriptor-declared token-endpoint authentication style, implement it in `HttpsOAuthTransport`, then admit Spotify. Design choice to settle during implementation: whether the style extends `ClientAuthentication` or becomes a separate closed descriptor field. The style is orthogonal to the PKCE-versus-secret question — Spotify is a confidential client sent via Basic — so a separate field is preferred to keep PKCE combinations expressible.

**Blocked by:** 21

**Status:** ready-for-agent

- [ ] Descriptor declares the token-endpoint auth style; validation is closed and refuses unknown values
- [ ] Basic style sends `Authorization: Basic base64(client_id:client_secret)` and omits the secret from the body
- [ ] Body style is unchanged; github, google, facebook, apple, linkedin, microsoft, gitlab, and discord flows do not regress
- [ ] `spotify` descriptor `1.0.0` admitted with its own scope ceiling and an end-to-end flow test asserting the Basic header on the exchange
- [ ] Credential bytes never appear in transport debug output or errors