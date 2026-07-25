"use strict";

function text(value, fallback) {
    try {
        if (value === null || value === undefined) return fallback;
        return String(value);
    } catch (_) {
        return fallback;
    }
}

function boolean(value, fallback) {
    try {
        return Boolean(value);
    } catch (_) {
        return fallback;
    }
}

function integer(value, fallback) {
    try {
        const number = Number(value);
        return Number.isFinite(number) ? Math.trunc(number) : fallback;
    } catch (_) {
        return fallback;
    }
}

function date(value) {
    try {
        if (!value) return null;
        return new Date(value).toISOString();
    } catch (_) {
        return null;
    }
}

function truncateText(value, limit) {
    const output = [];
    let truncated = false;
    for (const character of value) {
        if (output.length >= limit) {
            truncated = true;
            break;
        }
        output.push(character);
    }
    return {value: output.join(""), truncated: truncated};
}

function accountById(Mail, accountId) {
    const accounts = Mail.accounts();
    for (let index = 0; index < accounts.length; index += 1) {
        if (text(accounts[index].id(), "") === accountId) return accounts[index];
    }
    throw new Error("Mail account was not found");
}

function collectionChild(collection, name) {
    for (let index = 0; index < collection.length; index += 1) {
        if (text(collection[index].name(), "") === name) return collection[index];
    }
    throw new Error("Mailbox path was not found");
}

function resolveMailbox(Mail, reference) {
    const path = reference.path;
    if (!reference.account_id && path.length === 1 && path[0].toUpperCase() === "INBOX") {
        return Mail.inbox();
    }

    let collection = reference.account_id
        ? accountById(Mail, reference.account_id).mailboxes()
        : Mail.mailboxes();
    let mailbox = null;
    for (let index = 0; index < path.length; index += 1) {
        mailbox = collectionChild(collection, path[index]);
        if (index + 1 < path.length) collection = mailbox.mailboxes();
    }
    return mailbox;
}

function accountRecord(account) {
    let addresses = [];
    try {
        addresses = account.emailAddresses().map(String);
    } catch (_) {}
    return {
        id: text(account.id(), ""),
        name: text(account.name(), ""),
        email_addresses: addresses,
        enabled: boolean(account.enabled(), false),
    };
}

function mailboxRecords(account, accountId) {
    const output = [];

    function visit(mailboxes, parentPath) {
        for (let index = 0; index < mailboxes.length; index += 1) {
            const mailbox = mailboxes[index];
            const name = text(mailbox.name(), "");
            const path = parentPath.concat([name]);
            output.push({
                reference: {account_id: accountId, path: path},
                name: name,
                unread_count: Math.max(0, integer(mailbox.unreadCount(), 0)),
                unread_count_source: "mail_reported",
            });
            try {
                visit(mailbox.mailboxes(), path);
            } catch (_) {}
        }
    }

    visit(account.mailboxes(), []);
    return output;
}

function scopedMessageReference(message, fallback) {
    let accountId = fallback.account_id || null;
    let path = fallback.path;
    try {
        const mailbox = message.mailbox();
        accountId = text(mailbox.account().id(), "") || accountId;
    } catch (_) {}
    return {
        account_id: accountId,
        mailbox_path: path,
        id: integer(message.id(), 0),
    };
}

function messageRecord(message, reference, includeContent, maxBodyChars) {
    const record = {
        reference: scopedMessageReference(message, reference),
        message_id: text(message.messageId(), "") || null,
        subject: text(message.subject(), ""),
        sender: text(message.sender(), ""),
        date_received: date(message.dateReceived()),
        read: boolean(message.readStatus(), false),
        flagged: boolean(message.flaggedStatus(), false),
        size: Math.max(0, integer(message.messageSize(), 0)),
    };

    if (includeContent) {
        const content = text(message.content(), "");
        const truncated = truncateText(content, maxBodyChars);
        record.content = truncated.value;
        record.content_truncated = truncated.truncated;
    }
    return record;
}

function recordsFromColumns(columns, reference) {
    const names = [
        "ids",
        "message_ids",
        "subjects",
        "senders",
        "dates",
        "reads",
        "flags",
        "sizes",
    ];
    const length = columns.ids.length;
    for (const name of names) {
        if (!Array.isArray(columns[name]) || columns[name].length !== length) {
            throw new Error("Mail returned misaligned message columns");
        }
    }

    const records = [];
    for (let index = 0; index < length; index += 1) {
        records.push({
            reference: {
                account_id: reference.account_id || null,
                mailbox_path: reference.path,
                id: integer(columns.ids[index], 0),
            },
            message_id: text(columns.message_ids[index], "") || null,
            subject: text(columns.subjects[index], ""),
            sender: text(columns.senders[index], ""),
            date_received: date(columns.dates[index]),
            read: boolean(columns.reads[index], false),
            flagged: boolean(columns.flags[index], false),
            size: Math.max(0, integer(columns.sizes[index], 0)),
        });
    }
    return records;
}

function bulkMessageColumns(mailbox) {
    const messages = mailbox.messages;
    return {
        ids: messages.id(),
        message_ids: messages.messageId(),
        subjects: messages.subject(),
        senders: messages.sender(),
        dates: messages.dateReceived(),
        reads: messages.readStatus(),
        flags: messages.flaggedStatus(),
        sizes: messages.messageSize(),
    };
}

function bulkMessageRecords(mailbox, reference) {
    return recordsFromColumns(bulkMessageColumns(mailbox), reference);
}

function recordsFromAccountColumns(groups, path) {
    let output = [];
    for (const group of groups) {
        output = output.concat(recordsFromColumns(group.columns, {
            account_id: group.account_id,
            path: path,
        }));
    }
    return output;
}

function isAggregateInbox(reference) {
    return !reference.account_id &&
        reference.path.length === 1 &&
        reference.path[0].toUpperCase() === "INBOX";
}

function bulkRecordsForReference(Mail, reference) {
    if (!isAggregateInbox(reference)) {
        return bulkMessageRecords(resolveMailbox(Mail, reference), reference);
    }
    const groups = [];
    const accounts = Mail.accounts();
    for (let index = 0; index < accounts.length; index += 1) {
        const account = accounts[index];
        const accountReference = {
            account_id: text(account.id(), ""),
            path: ["INBOX"],
        };
        const mailbox = resolveMailbox(Mail, accountReference);
        groups.push({
            account_id: accountReference.account_id,
            columns: bulkMessageColumns(mailbox),
        });
    }
    return recordsFromAccountColumns(groups, ["INBOX"]);
}

function reportedUnreadCountForReference(Mail, reference) {
    if (!isAggregateInbox(reference)) {
        return Math.max(0, integer(resolveMailbox(Mail, reference).unreadCount(), 0));
    }
    let count = 0;
    const accounts = Mail.accounts();
    for (let index = 0; index < accounts.length; index += 1) {
        const accountReference = {
            account_id: text(accounts[index].id(), ""),
            path: ["INBOX"],
        };
        count += Math.max(0, integer(resolveMailbox(Mail, accountReference).unreadCount(), 0));
    }
    return count;
}

function receivedMillis(record) {
    if (!record.date_received) return null;
    const value = Date.parse(record.date_received);
    return Number.isFinite(value) ? value : null;
}

function compareAccountIds(left, right) {
    if (left === right) return 0;
    return left < right ? -1 : 1;
}

function compareNewestFirst(left, right) {
    const leftMillis = receivedMillis(left);
    const rightMillis = receivedMillis(right);
    if (leftMillis === null && rightMillis !== null) return 1;
    if (leftMillis !== null && rightMillis === null) return -1;
    if (leftMillis !== rightMillis) return rightMillis - leftMillis;
    if (left.reference.id !== right.reference.id) return right.reference.id - left.reference.id;
    const leftAccount = left.reference.account_id || "";
    const rightAccount = right.reference.account_id || "";
    return compareAccountIds(leftAccount, rightAccount);
}

function parseCursor(value) {
    if (!value) return null;
    const parts = value.split(":");
    if (parts.length !== 4 || parts[0] !== "v1") throw new Error("Search cursor was not valid");
    const millis = parts[1] === "none" ? null : Number(parts[1]);
    const id = Number(parts[2]);
    const accountId = decodeURIComponent(parts[3]);
    if ((millis !== null && !Number.isSafeInteger(millis)) ||
        !Number.isSafeInteger(id) ||
        id <= 0 ||
        !accountId) {
        throw new Error("Search cursor was not valid");
    }
    return {millis: millis, id: id, account_id: accountId};
}

function cursorFor(record) {
    const millis = receivedMillis(record);
    const accountId = record.reference.account_id || "";
    return "v1:" +
        (millis === null ? "none" : String(millis)) +
        ":" +
        String(record.reference.id) +
        ":" +
        encodeURIComponent(accountId);
}

function followsCursor(record, cursor) {
    if (!cursor) return true;
    const millis = receivedMillis(record);
    if (cursor.millis === null) {
        if (millis !== null) return false;
        if (record.reference.id !== cursor.id) return record.reference.id < cursor.id;
        return compareAccountIds(record.reference.account_id || "", cursor.account_id) > 0;
    }
    if (millis === null) return true;
    if (millis !== cursor.millis) return millis < cursor.millis;
    if (record.reference.id !== cursor.id) return record.reference.id < cursor.id;
    return compareAccountIds(record.reference.account_id || "", cursor.account_id) > 0;
}

function searchResult(records, args) {
    const senderNeedle = args.sender_contains ? args.sender_contains.toLowerCase() : null;
    const subjectNeedle = args.subject_contains ? args.subject_contains.toLowerCase() : null;
    const afterMillis = args.received_after ? Date.parse(args.received_after) : null;
    const beforeMillis = args.received_before ? Date.parse(args.received_before) : null;
    const cursor = parseCursor(args.cursor);
    const matches = records.filter(function(record) {
        if (args.unread !== null && args.unread !== undefined && (!record.read) !== args.unread) {
            return false;
        }
        if (args.flagged !== null && args.flagged !== undefined && record.flagged !== args.flagged) {
            return false;
        }
        if (senderNeedle && !record.sender.toLowerCase().includes(senderNeedle)) return false;
        if (subjectNeedle && !record.subject.toLowerCase().includes(subjectNeedle)) return false;
        const millis = receivedMillis(record);
        if (afterMillis !== null && (millis === null || millis < afterMillis)) return false;
        if (beforeMillis !== null && (millis === null || millis >= beforeMillis)) return false;
        return true;
    }).sort(compareNewestFirst);
    const remaining = matches.filter(function(record) { return followsCursor(record, cursor); });
    const messages = remaining.slice(0, args.limit);
    const hasMore = remaining.length > messages.length;
    return {
        messages: messages,
        scanned_count: records.length,
        matched_count: matches.length,
        has_more: hasMore,
        next_cursor: hasMore && messages.length > 0 ? cursorFor(messages[messages.length - 1]) : null,
        completeness: "complete",
    };
}

function inboxSnapshot(records, reportedUnreadCount, reference, args) {
    const newest = records.slice().sort(compareNewestFirst);
    const unread = newest.filter(function(record) { return !record.read; });
    return {
        mailbox: reference,
        total_count: records.length,
        unread_count: unread.length,
        unread_count_source: "exact_bulk_projection",
        mail_reported_unread_count: reportedUnreadCount,
        recent_messages: newest.slice(0, args.recent_limit),
        unread_messages: unread.slice(0, args.unread_limit),
        completeness: "complete",
    };
}

function findMessage(mailbox, id) {
    const candidate = mailbox.messages.byId(id);
    if (integer(candidate.id(), 0) !== id) throw new Error("Message was not found in the mailbox");
    return candidate;
}

function composeMessage(Mail, message, visible) {
    const outgoing = Mail.OutgoingMessage({
        subject: message.subject,
        content: message.body,
        visible: visible,
    });
    Mail.outgoingMessages.push(outgoing);
    for (const address of message.to) {
        outgoing.toRecipients.push(Mail.ToRecipient({address: address}));
    }
    for (const address of message.cc) {
        outgoing.ccRecipients.push(Mail.CcRecipient({address: address}));
    }
    for (const address of message.bcc) {
        outgoing.bccRecipients.push(Mail.BccRecipient({address: address}));
    }
    return outgoing;
}

function recipientAddresses(recipients) {
    const output = [];
    for (let index = 0; index < recipients.length; index += 1) {
        output.push(text(recipients[index].address(), ""));
    }
    return output.filter(function(address) { return address.length > 0; });
}

function replyContent(body, quotedContent) {
    const separator = body.length > 0 && quotedContent.length > 0 ? "\n\n" : "";
    return body + separator + quotedContent;
}

function normalizeMailContent(value) {
    return text(value, "").replace(/\r\n/g, "\n").replace(/\r/g, "\n");
}

function replyContentVerified(body, quotedContent, savedContent) {
    return normalizeMailContent(savedContent) ===
        normalizeMailContent(replyContent(body, quotedContent));
}

function outgoingMessageById(Mail, id) {
    const outgoing = Mail.outgoingMessages();
    for (let index = 0; index < outgoing.length; index += 1) {
        if (integer(outgoing[index].id(), 0) === id) return outgoing[index];
    }
    return null;
}

function dispatch(operation, args) {
    if (operation === "__health") return {status: "ready"};
    if (operation === "__test_truncate") return truncateText(args.text, args.limit);
    if (operation === "__echo") return args;
    if (operation === "__test_scope_reference") {
        const fakeMessage = {
            id: function() { return args.id; },
            mailbox: function() {
                return {
                    account: function() {
                        return {id: function() { return args.account_id; }};
                    },
                };
            },
        };
        return scopedMessageReference(fakeMessage, {
            account_id: args.fallback_account_id || null,
            path: args.path,
        });
    }
    if (operation === "__test_records_from_columns") {
        return recordsFromColumns(args.columns, args.reference);
    }
    if (operation === "__test_records_from_account_columns") {
        return recordsFromAccountColumns(args.groups, args.path);
    }
    if (operation === "__test_search_records") {
        return searchResult(args.records, args.request);
    }
    if (operation === "__test_inbox_snapshot_records") {
        return inboxSnapshot(
            args.records,
            args.reported_unread_count,
            args.reference,
            args.request
        );
    }
    if (operation === "__test_reply_content") {
        return replyContent(args.body, args.quoted_content);
    }
    if (operation === "__test_reply_content_verified") {
        return replyContentVerified(args.body, args.quoted_content, args.saved_content);
    }

    const Mail = Application("Mail");

    if (operation === "list_accounts") {
        return Mail.accounts().map(accountRecord);
    }

    if (operation === "list_mailboxes") {
        const accounts = args.account_id ? [accountById(Mail, args.account_id)] : Mail.accounts();
        let output = [];
        for (let index = 0; index < accounts.length; index += 1) {
            const account = accounts[index];
            output = output.concat(mailboxRecords(account, text(account.id(), "")));
        }
        return output;
    }

    if (operation === "search_messages") {
        return searchResult(bulkRecordsForReference(Mail, args.mailbox), args);
    }

    if (operation === "inbox_snapshot") {
        const reference = {account_id: args.account_id || null, path: ["INBOX"]};
        return inboxSnapshot(
            bulkRecordsForReference(Mail, reference),
            reportedUnreadCountForReference(Mail, reference),
            reference,
            args
        );
    }

    if (operation === "get_message") {
        const mailbox = resolveMailbox(Mail, {
            account_id: args.message.account_id,
            path: args.message.mailbox_path,
        });
        const message = findMessage(mailbox, args.message.id);
        return messageRecord(
            message,
            {account_id: args.message.account_id, path: args.message.mailbox_path},
            true,
            args.max_body_chars
        );
    }

    if (operation === "check_mail") {
        if (args.account_id) {
            Mail.checkForNewMail({for: accountById(Mail, args.account_id)});
        } else {
            Mail.checkForNewMail();
        }
        return {requested: true};
    }

    if (operation === "set_message_state") {
        const mailbox = resolveMailbox(Mail, {
            account_id: args.message.account_id,
            path: args.message.mailbox_path,
        });
        const message = findMessage(mailbox, args.message.id);
        if (args.read !== null && args.read !== undefined) message.readStatus = args.read;
        if (args.flagged !== null && args.flagged !== undefined) message.flaggedStatus = args.flagged;
        return {
            message: args.message,
            read: boolean(message.readStatus(), false),
            flagged: boolean(message.flaggedStatus(), false),
        };
    }

    if (operation === "move_message") {
        const source = resolveMailbox(Mail, {
            account_id: args.message.account_id,
            path: args.message.mailbox_path,
        });
        const message = findMessage(source, args.message.id);
        const destination = resolveMailbox(Mail, args.destination);
        Mail.move(message, {to: destination});
        return {
            moved: true,
            destination: args.destination,
        };
    }

    if (operation === "create_draft") {
        const draft = composeMessage(Mail, args.message, true);
        Mail.save(draft);
        return {
            local_id: integer(draft.id(), 0),
            sent: false,
            visible: boolean(draft.visible(), false),
        };
    }

    if (operation === "create_reply_draft") {
        const mailbox = resolveMailbox(Mail, {
            account_id: args.message.account_id,
            path: args.message.mailbox_path,
        });
        const source = findMessage(mailbox, args.message.id);
        const reply = Mail.reply(source, {openingWindow: true, replyToAll: false});
        const quotedContent = text(reply.content(), "");
        reply.content = replyContent(args.body, quotedContent);
        Mail.save(reply);
        const localId = integer(reply.id(), 0);
        const persisted = outgoingMessageById(Mail, localId);
        const saved = persisted || reply;
        const savedContent = text(saved.content(), "");
        const draftPresent = persisted !== null;
        return {
            source: scopedMessageReference(source, {
                account_id: args.message.account_id,
                path: args.message.mailbox_path,
            }),
            local_id: localId,
            subject: text(saved.subject(), ""),
            to: recipientAddresses(saved.toRecipients()),
            cc: recipientAddresses(saved.ccRecipients()),
            sent: !draftPresent,
            draft_present: draftPresent,
            visible: boolean(saved.visible(), false),
            content_verified: replyContentVerified(args.body, quotedContent, savedContent),
        };
    }

    if (operation === "send_message") {
        const outgoing = composeMessage(Mail, args.message, false);
        const localId = integer(outgoing.id(), 0);
        const sent = boolean(Mail.send(outgoing), false);
        return {local_id: localId, sent: sent, visible: false};
    }

    throw new Error("Unsupported automation operation");
}

function run(argv) {
    try {
        const operation = argv[0];
        const args = JSON.parse(argv[1]);
        return JSON.stringify({ok: true, data: dispatch(operation, args)});
    } catch (error) {
        return JSON.stringify({ok: false, error: {message: text(error, "Automation failed")}});
    }
}
