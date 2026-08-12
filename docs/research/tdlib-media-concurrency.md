# TDLib media concurrency

Research snapshot: 2026-08-11

## Answer

Yes. Telegram connections can be parallelized, and TDLib does so deliberately. It does **not** put media behind the same serial request slot as ordinary API traffic. For every initialized data center, current TDLib creates distinct `SessionMultiProxy` pools for ordinary RPCs, uploads, large downloads, and small downloads. `NetQueryDispatcher` routes `Common`, `Upload`, `Download`, and `DownloadSmall` queries to those four pools ([`NetQueryDispatcher.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/net/NetQueryDispatcher.cpp)).

Current `master` configures two large-download sessions and two small-download sessions per DC for an ordinary account, or eight of each for Premium. Uploads use four sessions for non-Premium accounts in DC 2/4 and eight otherwise; ordinary-RPC session count is a separate option with a minimum of one. These are logical MTProto sessions and pool capacities, not a promise that every TCP socket is eagerly connected and permanently open. Telegram's protocol guide describes each transfer session as an additional `session_id` over the same authorization key, without reauthorization, and recommends separate connections so bulk traffic does not interfere with updates or ordinary RPCs ([TDLib pool construction](https://github.com/tdlib/td/blob/master/td/telegram/net/NetQueryDispatcher.cpp), [Telegram file-transfer guidance](https://core.telegram.org/api/files#general-considerations)).

The resulting model is both kinds of multiplexing:

- Within one `Session`, several requests share one MTProto connection. TDLib queues pending queries, tracks all outstanding queries in `sent_queries_` by MTProto message ID, can place multiple messages in one MTProto container, and resolves responses independently by their message IDs ([`Session.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/net/Session.cpp)).
- Across a `SessionMultiProxy` pool, work is spread over several logical sessions, each able to own a separate connection. The file-transfer guide explicitly recommends multiple parallel call queues linked to separate TCP connections ([`SessionMultiProxy.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/net/SessionMultiProxy.cpp), [Telegram upload guidance](https://core.telegram.org/api/files#uploading-files)).

## Download scheduling and backpressure

`FileDownloader` is an actor for one file, but it is not a one-RPC-at-a-time loop. While its byte resource allowance has room for another part, it starts a part, assigns a local unique ID, stores the part and cancellation signal in `part_map_`, and dispatches the query asynchronously. Completions release the part's resource usage and drive the loop again. Secret-chat encryption is an exception that requires ordered processing ([part creation and routing](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L120-L176), [bounded dispatch loop](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L535-L589), [completion correlation](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L613-L648)).

The allowance comes from `ResourceManager`. Each file worker registers with a priority; the manager aggregates active and estimated byte demand, rounds grants to the file's part size, never grants beyond its unused budget, and notifies the worker when capacity changes. In baseline mode it visits workers in priority order; greedy mode orders by estimated remaining demand ([`ResourceManager.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/files/ResourceManager.cpp)). `FileDownloader::update_priority` forwards a priority change to that manager ([`FileDownloader.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L398-L410)). This is explicit admission/backpressure, rather than an unbounded queue of spawned downloads.

Telegram also publishes per-DC soft limits on the number of concurrently active files: separate limits for files below and above 20 MiB. The current example configuration reports five small files and two large files, but these are server configuration values and clients should consume them instead of assuming they are permanent constants ([download parallelism rule](https://core.telegram.org/api/files#downloading-files), [configuration fields and current example](https://core.telegram.org/api/config#small-queue-max-active-operations-count)).

## Chunks, media DCs, and CDN redirects

TDLib sends `upload.getFile` to the DC recorded in the file location and marks the query as either `DownloadSmall` or `Download`. A downloader may have multiple part requests outstanding; each request asks for one aligned range. Telegram currently permits a part up to 1 MiB and requires a part to remain inside one 1-MiB boundary, with alignment rules depending on `precise` ([TDLib request construction](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L130-L176), [`upload.getFile`](https://core.telegram.org/method/upload.getFile), [Telegram chunk rules](https://core.telegram.org/api/files#downloading-files)).

If a matching `dcOption` is marked `media_only`, Telegram requires the file query to use it. TDLib's connection creator accepts `allow_media_only` and `is_media`, keeps client state keyed by a stable client hash, and returns connections through the same per-DC machinery ([media-DC rule](https://core.telegram.org/api/files#general-considerations), [`ConnectionCreator.cpp`](https://github.com/tdlib/td/blob/master/td/telegram/net/ConnectionCreator.cpp#L534-L555)). An authorization key is managed per DC and reused by that DC's session pools; creating a new socket for every file is not the intended design.

TDLib advertises CDN support on ordinary file requests. On `upload.fileCdnRedirect`, it records the external CDN DC, token, AES key/IV, and hashes, then issues `upload.getCdnFile` to the CDN without user authorization. It decrypts each part and verifies the server-provided hashes; reupload requests go back to the original master DC. Invalid tokens fall back to the master DC ([TDLib redirect state machine](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L54-L118), [TDLib CDN request construction](https://github.com/tdlib/td/blob/master/td/telegram/files/FileDownloader.cpp#L135-L174), [Telegram CDN protocol](https://core.telegram.org/cdn)).

## Implications for Intuigram

Intuigram's single Telegram actor is compatible with parallel networking only if it owns a continuously polled connection/session driver and accepts several correlated invocations at once. The implementation following this research now does that: the application has independent bounded control, small-media, and large-transfer admission; the Account actor owns persistent per-DC media sessions; and `compio-mtproto` correlates bounded concurrent requests while its passive update stream keeps progressing ([runtime lanes](../../crates/intuigram-app/src/runtime/mod.rs), [media sessions](../../crates/intuigram-telegram/src/media/session/mod.rs), [preview admission](../../crates/intuigram-lib/src/app/media_preview.rs), [avatar admission](../../crates/intuigram-lib/src/app/avatar_loads.rs)).

The implemented TDLib-shaped design is:

1. Keep one Account-owned Telegram actor and worker-local driver; do not create a new actor or authorization for every image.
2. Give media an independent bounded effect class, with correlated completion IDs and cancellation, so ordinary RPC/update progress is not behind file bytes.
3. Maintain reusable per-DC small-download and large-download session pools. A session may multiplex several requests on one connection; the pool may open several connections when useful.
4. Admit several visible small files concurrently per DC, capped by Telegram's live `small_queue_max_active_operations_count`, with active-chat/avatar work ahead of background prefetch. Use the large-file limit separately.
5. Within a file, permit several aligned part requests only under an explicit byte budget. Preserve ordered handling where encryption requires it.
6. Reuse imported per-DC authorization and persistent connections, and add the CDN redirect/decrypt/hash path rather than reconnecting and reauthorizing for each item.

For the avatar/thumbnail symptom, steps 1-4 matter most. More renderer workers cannot remove network head-of-line blocking while state and orchestration admit only one Telegram media effect.
