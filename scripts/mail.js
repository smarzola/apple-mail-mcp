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
    if (!accountId) {
        try {
            const mailbox = message.mailbox();
            accountId = text(mailbox.account().id(), "") || null;
        } catch (_) {}
    }
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
            account_id: null,
            path: args.path,
        });
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
        const mailbox = resolveMailbox(Mail, args.mailbox);
        const output = [];
        const scanLimit = Math.min(mailbox.messages.length, 1000);
        const senderNeedle = args.sender_contains ? args.sender_contains.toLowerCase() : null;
        const subjectNeedle = args.subject_contains ? args.subject_contains.toLowerCase() : null;

        for (let index = 0; index < scanLimit && output.length < args.limit; index += 1) {
            const message = mailbox.messages.at(index);
            const read = boolean(message.readStatus(), false);
            const flagged = boolean(message.flaggedStatus(), false);
            const sender = text(message.sender(), "");
            const subject = text(message.subject(), "");
            if (args.unread !== null && args.unread !== undefined && (!read) !== args.unread) continue;
            if (args.flagged !== null && args.flagged !== undefined && flagged !== args.flagged) continue;
            if (senderNeedle && !sender.toLowerCase().includes(senderNeedle)) continue;
            if (subjectNeedle && !subject.toLowerCase().includes(subjectNeedle)) continue;
            output.push(messageRecord(message, args.mailbox, false, 0));
        }
        return output;
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
