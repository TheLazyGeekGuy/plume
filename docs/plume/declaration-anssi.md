# Déclaration de fourniture d'un moyen de cryptologie : Plume

Les réponses à reporter dans le dossier ANSSI (démarche numérique). Les champs entre crochets sont à toi : identité, adresse, signature. Les intitulés suivent le contenu prévu par l'arrêté du 29 janvier 2015. Adapte-les aux libellés exacts du formulaire.

## 1. Déclarant (fournisseur)

- Nom, prénom ou raison sociale : [à remplir]
- Adresse, courriel, téléphone : [à remplir]
- Qualité : développeur et distributeur du logiciel (fourniture en France, distribution gratuite en téléchargement)

## 2. Identification du moyen

- Nom commercial : Plume
- Nature : logiciel, traitement de texte (édition de documents Markdown et Word), libre et gratuit
- Version déclarée : 0.8.0
- Licence : MIT ou Apache-2.0, au choix
- Plateformes : Windows, macOS, Linux, BSD ; version web (WebAssembly, dans le navigateur)
- Code source : public, dépôt GitHub [URL publique du dépôt à confirmer : `https://github.com/TheLazyGeekGuy/plume`]
- Compilation : Rust stable (`rustc` 1.97), `cargo build --release -p wordcraft` ; web : `cargo xtask wasm`
- Mode de distribution : téléchargement libre et gratuit ; aucun service en ligne ni serveur de clés fourni

## 3. Finalité des fonctions de cryptologie

Protéger la **confidentialité** et l'**intégrité** des documents de l'utilisateur, enregistrés sur son propre disque. Le chiffrement et le déchiffrement se font uniquement sur son poste, avec un mot de passe qu'il choisit. Plume ne transmet, ne séquestre et ne récupère aucune clé. Sans le mot de passe, le document ne peut pas être ouvert.

L'utilisateur ne peut pas modifier les fonctions cryptographiques : il choisit seulement le mot de passe et le format.

## 4. Fonctions de cryptologie

### 4.1 Chiffrement de fichiers au format AES Crypt (`.aes`), format publié

| Élément | Valeur |
|---|---|
| Chiffrement | AES-256, mode CBC, IV aléatoire de 128 bits |
| Dérivation de clé | PBKDF2-HMAC-SHA512, sel aléatoire de 128 bits, 300 000 itérations (format 3) |
| Clé de session | 256 bits aléatoires, chiffrée par la clé dérivée du mot de passe |
| Intégrité | HMAC-SHA256 sur l'en-tête de session et sur les données |
| Lecture | formats 2 et 3 (format 2 : dérivation SHA-256 itérée 8 192 fois) |
| Aléa | générateur du système (`getrandom`) ; navigateur : `crypto.getRandomValues` |

### 4.2 ZIP chiffré, méthode WinZip AES (AE-2), format publié

| Élément | Valeur |
|---|---|
| Chiffrement | AES-256, mode CTR, par fichier de l'archive |
| Dérivation de clé | PBKDF2-HMAC-SHA1, 1 000 itérations, sel de 128 bits (paramètres du format) |
| Intégrité | HMAC-SHA1 tronqué à 80 bits par fichier |
| Lecture seule | ZipCrypto (ancien chiffrement ZIP), jamais écrit |

### 4.3 Documents Word protégés par mot de passe (spécification publique MS-OFFCRYPTO)

| Élément | Valeur |
|---|---|
| `.docx` en écriture et lecture | chiffrement « agile » : AES-256-CBC, SHA-512 itéré 100 000 fois avec sel, HMAC sur le flux chiffré |
| `.doc` (Word 97-2003), lecture seule | obfuscation XOR, RC4 (clé dérivée par MD5), RC4 CryptoAPI (SHA-1, clé de 40 à 128 bits) |

### 4.4 Communications

Les connecteurs de forges (GitHub, GitLab, Forgejo) utilisent HTTPS (TLS) par la bibliothèque `rustls`, avec vérification des certificats par le système. Plume n'implémente pas de protocole de chiffrement des communications qui lui soit propre.

## 5. Composants qui réalisent les fonctions (bibliothèques libres)

| Composant | Version | Rôle |
|---|---|---|
| `aes` (RustCrypto) | 0.8.4 / 0.9.3 | AES-256 |
| `sha2` (RustCrypto) | 0.11.0 | SHA-256, SHA-512 |
| `sha1` (RustCrypto) | 0.10.7 / 0.11.0 | SHA-1 (ZIP AES, lecture `.doc`) |
| `hmac` (RustCrypto) | 0.12.1 / 0.13.0 | HMAC |
| `pbkdf2` (RustCrypto) | 0.12.2 / 0.13.0 | PBKDF2 |
| `zip` | 3.0.0 | ZIP AES (AE-2) |
| `zeroize` | 1.9.1 | effacement des clés en mémoire |
| `getrandom` | 0.2 à 0.4 | aléa système |
| `rustls` (par `ureq` 3.4) | — | TLS des connecteurs |

Code propre à Plume : `crates/crypt` (format AES Crypt, enveloppe ZIP AES), `crates/docx/src/crypt.rs` (Word agile), `crates/docbin/src/crypt.rs` (lecture `.doc`). RC4 et le mode CBC sont implémentés dans ce code ; aucun algorithme n'a été inventé.

## 6. Classement demandé

Moyen **« grand public »** (décret n° 2007-663, annexe II, point 3) :

1. Il est disponible sans restriction : téléchargement libre et gratuit.
2. L'utilisateur ne peut pas modifier sa fonction cryptographique.
3. Il s'installe et s'utilise sans aide importante du fournisseur.

## 7. Pièces jointes

- Code source, ou l'URL du dépôt avec le commit déclaré : [hash du commit de la version déclarée]
- Ce document, en description technique
- Documentation : `docs/plume/chiffrement-transport.md`
