# PDFPilot review service

This is an independent, functional local service with persistent files and a browser UI. It is not deployed publicly or ready for an unreviewed public production rollout.

## Storage and authentication

`config.json` holds the administrator token. Each review has a `source.pdf` snapshot and `state.json` containing its document digest, private review/recipient tokens, comments, recipients and event history. Storage is private on Unix (folder mode 0700; JSON writes use private temporary files). Do not point the service at an unrelated shared folder.

Private links have the form `/review/<id>#<token>`. The token stays in the browser fragment and is sent as an Authorization bearer header for API calls. The PDF is fetched with that header and displayed through a local blob URL. Possession of a recipient token grants that recipient's signing authority; it does not establish their real-world identity. Tokens currently have no expiry/revocation UI.

The server binds loopback only. Remote deployment needs a same-host TLS reverse proxy, backups, an appropriate hosting/security review, and a remote desktop publication client or deployment-specific publisher. Never expose the storage directory through static web hosting.

## API

All API routes require `Authorization: Bearer <token>`. Review/recipient tokens are scoped to one review; the administrator token can access any review. Creation is currently performed by the desktop/local `Service::publish` API.

| Method | Route | Operation |
| --- | --- | --- |
| GET | `/review/<id>` | Browser shell; data/PDF still require a token |
| GET | `/api/reviews/<id>` | Status, comments and events; private tokens are omitted |
| GET | `/api/reviews/<id>/document.pdf` | Published snapshot |
| POST | `/api/reviews/<id>/comments` | JSON: `page`, `author`, `text` |
| POST | `/api/reviews/<id>/resolve` | JSON: `comment_id`, `resolved` |
| POST | `/api/reviews/<id>/sign` | JSON: `typed_name`, `consent: true`; requires the recipient's token |
| GET | `/api/reviews/<id>/completed.pdf` | Original plus receipt, after every recipient has signed |

Desktop reminders append activity events; no email/SMS provider is configured. The browser and desktop poll for activity while running. Comments and signing events are service records, separate from the source PDF's annotations.

## Integrity and operational limits

Signing order, duplicate signing, authorization, page bounds and payload sizes are checked. HTTP requests are bounded to 8 KiB headers/64 KiB bodies with timeouts; chunked transfer is refused. The server handles requests serially. It has no production rate limiter, multi-process storage locking, account system, email verification, tenant model or independently trusted timestamps.

Events include a SHA-256 chain and the original PDF has a stored digest. These detect accidental/individual event changes and snapshot changes. They do not prevent a privileged operator from rebuilding the complete history. Audit assurance needs independent anchoring and operational controls.

Completed PDFs include a signing receipt and an attached UTF-8 signing record with private access tokens omitted. Non-ASCII characters appear as Unicode codepoints in the receipt. Publishing and typed signing do not apply a certificate signature. The desktop's separate certificate workflow creates detached CMS PDF signatures with OpenSSL. No automatic outbound messages, uploads or external verification calls are made by this service.
