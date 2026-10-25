# 62 [R]: Descriptor-declared token-endpoint authentication and Spotify

**What to build:** Every maintained provider authenticates to the token endpoint by placing
`client_secret` in the request body. Spotify authenticates with HTTP
`Authorization: Basic base64(client_id:client_secret)` and rejects body secrets, which is why
it had no descriptor: a `spotify` descriptor would parse, validate, and then fail at the
exchange. Add a descriptor-declared token-endpoint authentication style, implement it in
`HttpsOAuthTransport`, then admit Spotify. Design choice to settle during implementation:
whether the style extends `ClientAuthentication` or becomes a separate closed descriptor field.
The style is orthogonal to the PKCE-versus-secret question — Spotify is a confidential client
sent via Basic — so a separate field is preferred to keep the two questions apart.

**Outcome:** the style is a separate closed descriptor field, `TokenEndpointAuthentication`,
with two values: `RequestBody` and `Basic`. The transport honours both — `Basic` sends the
`Authorization: Basic base64(client_id:client_secret)` header and withholds the secret from the
body, `RequestBody` does the opposite. Every maintained descriptor declares `RequestBody`,
including Spotify, whose token endpoint documents `client_secret_post` among the methods it
accepts; the field stays so a provider that accepts only HTTP Basic is expressible without
another transport change. Spotify joins the catalog as the ninth provider on its full OpenID
Connect surface (`https://accounts.spotify.com`, key set at
`https://accounts.spotify.com/oidc/certs/v1`), with its own scope ceiling of `email`, `openid`,
and `profile`.

**Blocked by:** 21

**Status:** done

- [x] Descriptor declares the token-endpoint auth style; validation is closed and refuses unknown values
- [x] Basic style sends `Authorization: Basic base64(client_id:client_secret)` and omits the secret from the body
- [x] Body style is unchanged; github, google, facebook, apple, linkedin, microsoft, gitlab, and discord flows do not regress
- [x] `spotify` descriptor `1.0.0` admitted with its own scope ceiling (`email`, `openid`, `profile`)
- [x] Credential bytes never appear in transport debug output or errors
