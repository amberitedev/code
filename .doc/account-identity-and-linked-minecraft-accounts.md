# Account Identity and Linked Minecraft Accounts

Status: proposed foundation for implementation

## Purpose

Amberite needs one understandable identity model for desktop and web users. A person starts by signing in with Minecraft. Their primary Minecraft account anchors their Amberite profile; any additional Minecraft accounts are linked identities under that profile.

The internal Amberite profile identifier remains necessary for storing cores, friendships, settings, and service connections. It is not a separate user-facing login or an identity the user is expected to have created.

## Product language

Use the terms below consistently.

| Term | Meaning | User-facing use |
| --- | --- | --- |
| Primary Minecraft account | The Minecraft identity that created and anchors an Amberite profile. | "Signed in as <Minecraft username>" and "Primary Minecraft account". |
| Linked Minecraft account | An additional verified Minecraft identity that can access the same Amberite profile. | "Linked Minecraft accounts". |
| Amberite profile | The profile and data container owned by the primary Minecraft account. | Settings, friendships, cores, and optional service connections. |
| Amberite account | The internal account/profile record. | Avoid in the app. It may be used in web/product copy where a product-account term is needed, but it must always make clear which primary Minecraft account it is associated with. |

Do not ask users to create an Amberite username or password. That would imply a standalone identity system that does not exist in this version.

## Identity hierarchy

```text
Primary Minecraft account
└── Amberite profile
    ├── Cores and server ownership
    ├── Friends and social data
    ├── Profile settings
    ├── Optional Modrinth connection
    └── Linked Minecraft accounts
        ├── Secondary Minecraft account A
        └── Secondary Minecraft account B
```

The first verified Minecraft account is the primary account. Secondary accounts have equal access after being linked, but are lower in the presentation and ownership hierarchy: they point back to the one profile rooted in the primary account.

## Authentication and account creation

Authentication uses Microsoft OAuth and verified Minecraft ownership. Amberite must not collect or store a Microsoft/Minecraft password.

For version one, a user must own a verified Minecraft account to create or access an Amberite profile. This keeps the product model clear: Amberite is for people who use Minecraft accounts, and every profile has a meaningful primary identity.

A future "server organizer" or guardian role without Minecraft ownership can be added as a separate access-control feature. It must not be quietly treated as another kind of primary Minecraft profile; that would create two incompatible root-account concepts.

### First sign-in

```text
User selects "Sign in with Minecraft"
→ Microsoft OAuth authorization
→ Amberite verifies Minecraft ownership and canonical account identity
→ No Amberite profile exists for this Minecraft account
→ Create the profile with this account as primary
→ Enter the app/dashboard as that Minecraft account
```

The UI should state what happened plainly: "Your Amberite profile was created for <Minecraft username>."

### Sign-in with a linked secondary account

```text
User signs in with a verified linked Minecraft account
→ Resolve its parent Amberite profile
→ Enter that profile
→ Clearly show: "Signed in as <secondary username>"
```

Linked accounts are full login identities for the profile. Therefore compromise of any linked Microsoft/Minecraft account grants access to the shared profile. Linking and unlinking require recent reauthentication.

## Linking another Minecraft account

Only a user already signed in to an Amberite profile can start linking.

```text
Settings → Linked Minecraft accounts → Link account
→ Sign in through Microsoft OAuth for the additional Minecraft account
→ Verify Minecraft ownership
→ Reauthenticate the profile's primary Minecraft account
→ Add the verified account as a linked Minecraft account
→ Confirm the account, UUID, current username, and skin/avatar
```

Reauthenticating the primary account prevents an unattended session from being used to attach an identity without the profile owner's approval.

The initial version does not support moving a primary Minecraft account between profiles, merging profiles, or promoting a linked account to primary. These operations can transfer server ownership and social data and require a dedicated recovery/migration design.

## When a Minecraft account is already associated with a profile

```text
User signs in with Minecraft account X
→ X is already primary for an Amberite profile
→ Offer "Open that profile" or "Cancel"

User tries to link Minecraft account X
→ X is already associated with any Amberite profile
→ Explain that it cannot be linked or moved here
→ Offer "Open that profile" or "Cancel"
```

Never present a one-click merge, transfer, or "link anyway" action. The user should never be surprised by identity or ownership movement during login.

## Profile display and Settings UI

The Account section must match the width, density, and content sizing of the other Settings sections. It should not use a special larger layout or modal sizing.

Use the existing avatar/icon treatment from the instance Settings client-type control as the visual template for Minecraft skin/avatar images. Do not introduce a separate avatar design.

The initial Account section should display:

- Primary Minecraft avatar and current Minecraft username.
- A clear primary-account label.
- Linked Minecraft accounts, each with its avatar, current username, and link-management actions.
- The currently authenticated Minecraft account when it differs from the primary account.
- Optional connected-service state, including Modrinth when applicable.

Minecraft usernames are display data, not immutable identifiers. Store and resolve accounts by their canonical Minecraft UUID (and the appropriate Microsoft/Xbox identity used by the authentication flow), then refresh the current username and skin as needed.

## Friends and discovery

In the initial version, friend discovery uses Minecraft usernames only.

```text
Search exact Minecraft username
→ Resolve the linked or primary Minecraft identity
→ Resolve its Amberite profile
→ Show the matched Minecraft account, avatar, and the profile's primary account
→ Confirm and send a request to the Amberite profile
```

Friendship belongs to Amberite profiles, not individual Minecraft accounts. Searching for any linked Minecraft account finds the same profile. Showing the matched account before confirmation reduces mistaken requests when names are similar.

Future identity fields, deliberately deferred:

- Amberite handle: a unique, intentional product identifier if Amberite later needs an identity independent of Minecraft.
- Display name: a non-unique public profile name.
- Nickname: a private, per-user label for a friend; it is not a public identity.

Do not add these fields until there is a concrete social/profile need. An early Amberite handle would look like an unexplained extra account username.

## Modrinth connection

Modrinth is an optional connected service, not an identity root and not part of the login hierarchy.

```text
Amberite profile
└── Optional Modrinth connection
    └── Enables Modrinth content/integration where needed
```

The UI should explain that connecting Modrinth authorizes Amberite to use the user's Modrinth content; it does not change which Minecraft account owns or signs in to the Amberite profile.

## Decisions deferred for a later design

- A recovery and migration path for a lost primary Minecraft account.
- Promoting a linked account to primary, profile merging, and identity transfer.
- Non-Minecraft web dashboard/guardian accounts.
- Public profiles, Amberite handles, display names, and nicknames.
- Visibility controls for linked Minecraft accounts.
- Whether a primary account may be unlinked, and the deletion/retention policy for an Amberite profile.

## Implementation guardrails

- Treat OAuth authorization and Minecraft ownership verification as the source of identity; never accept or persist user passwords.
- Require recent primary-account reauthentication before linking or unlinking identities and before other destructive profile operations.
- Use immutable canonical IDs for authorization and storage; treat usernames and skins as mutable presentation fields.
- Make every account-selection screen say whether the selected Minecraft account will create, open, or link to an Amberite profile.
- Keep login/account decisions out of generic modals where possible; the screen must expose the target Minecraft account and the consequence of the selected action.
