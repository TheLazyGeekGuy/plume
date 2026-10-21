# Plume : chiffrement, transport et déploiement

> **Date :** 2026-10-21 · **Statut :** priorités 1 à 3 livrées (moteur de transport, AES Crypt et ZIP AES-256, indicateur et enregistrement chiffré) ; age, 7z et le déploiement vers les forges restent à faire · **Base :** branche `claude/gallant-allen-2rb6nx`

## 1. Ce qui est livré

### 1.0 Désactivé par défaut, en attendant la réponse de l'ANSSI

Le chiffrement n'est compilé qu'avec l'option `encryption`. **Par défaut, elle est désactivée** : un exécutable ou une version web construits normalement (`cargo build --release -p wordcraft`, `cargo xtask wasm`, la CI de release) ne contiennent aucun chiffrement de fichiers ni de documents. Ils n'ont pas les crates `aes`, `pbkdf2` et `hmac`, ni le RC4 de Plume, ni la partie AES de la crate `zip` (vérifié avec `cargo tree`).

Sans l'option :

- un fichier chiffré (`.aes`, ZIP chiffré, document Word ou `.doc` protégé par mot de passe) est reconnu, et Plume dit qu'il ne peut pas l'ouvrir, sans demander de mot de passe ;
- enregistrer avec un mot de passe, `crypto.encryptFile`, `crypto.decryptFile` ou un `transport.convert` avec mot de passe renvoient une erreur, et rien n'est écrit ;
- l'interface ne montre ni le cadenas, ni « Protéger le document », ni les formats `.md.aes` et `.textpack.aes`.

Restent dans tous les builds : HTTPS des connecteurs de forges (`rustls` avec `ring`, qui contient lui-même AES et ChaCha20 pour TLS ; la version web passe par le HTTPS du navigateur), les empreintes SHA des mots de passe de « Restreindre la modification » et la détection des formats chiffrés. Seul TLS chiffre, et seulement les communications.

Pour construire une version avec chiffrement : `cargo build --release -p wordcraft --features encryption`. Même option pour `wordcraft-cli` et `wordcraft-web`. `cargo xtask ci` teste les deux modes.

Le code reste dans le dépôt public. Savoir si publier ce seul code source compte déjà comme une fourniture est la première question posée à l'ANSSI.

### 1.1 Un document = un paquet

Un document voyage sous forme de **paquet** : son fichier Markdown et les fichiers à côté de lui (images, pièces jointes, tout ce que l'archive contenait), chacun avec son chemin. Le paquet existe sous quatre formes :

| Forme | Fichier | Contenu |
|---|---|---|
| Markdown + ressources | `rapport.md` dans un dossier | le Markdown et les fichiers locaux qu'il cite |
| ZIP | `rapport.zip` | les mêmes fichiers, aux mêmes chemins |
| TextPack | `rapport.textpack` | `text.md`, `info.json`, fichiers sous `assets/` |
| TextBundle | dossier `rapport.textbundle/` | idem, en dossier |

`transport.convert` passe d'une forme à l'autre **octet pour octet** : le Markdown n'est pas réécrit par l'éditeur, et les fichiers que rien ne cite sont gardés. Pour un TextBundle, les fichiers hors de `assets/` y sont déplacés, les liens vers eux mis à jour, et les chemins d'origine notés dans `info.json` (clé `net.plume.transport`) : le retour vers Markdown ou ZIP rend exactement le paquet de départ. Si ce déplacement ne peut pas être défait à l'identique, les fichiers restent où les liens les trouvent.

Les liens distants (`https://…`) sont listés dans le résultat (`links.remote`), **jamais téléchargés**. Les liens qui sortent du dossier (`../logo.png`) sont listés à part et ne sont pas emportés.

Tests : `md+assets → TextPack → ZIP → md+assets` et `md+assets → ZIP → chiffrement → déchiffrement → extraction` rendent le même paquet (`crates/engine/src/transport.rs`).

### 1.2 Chiffrement

Aucun algorithme n'est inventé : Plume écrit des formats publiés que d'autres outils ouvrent.

| Méthode | Fichier | Algorithmes | Ce qui reste visible |
|---|---|---|---|
| AES Crypt, format 3 (écrit), formats 2 et 3 (lus) | `rapport.md.aes`, `rapport.textpack.aes`, `rapport.zip.aes`, tout fichier `.aes` | clé PBKDF2-HMAC-SHA512 (300 000 itérations par défaut), AES-256-CBC, HMAC-SHA256 | le nom du fichier et sa taille approximative |
| ZIP AES-256 (WinZip AE-2) | `rapport.zip` avec mot de passe | clé PBKDF2-HMAC-SHA1, AES-256-CTR, HMAC-SHA1 par entrée | les noms, tailles et dates des fichiers du ZIP |
| ZipCrypto (ancien ZIP chiffré) | lecture seule | faible | — |

- AES Crypt est écrit par Plume (crate `wordcraft-crypt`) à partir des primitives RustCrypto. Il est vérifié contre des fichiers produits par l'outil AES Crypt (formats 2 et 3) et, dans les deux sens, contre une implémentation indépendante (`aescrypt-rs`).
- ZIP AES-256 passe par l'implémentation WinZip AES de la crate `zip`. Ses auteurs indiquent qu'elle n'a pas fait l'objet d'une revue cryptographique ; c'est à garder en tête pour un usage sensible. AES Crypt autour d'un ZIP (`.zip.aes`) masque aussi les noms de fichiers.
- **L'intégrité est vérifiée avant tout succès** : les deux HMAC d'AES Crypt, le HMAC de chaque entrée ZIP. Un mauvais mot de passe ou un fichier modifié donne une erreur, jamais un contenu faux. Chaque fichier écrit est relu et déchiffré avant d'être déclaré enregistré.
- **Les mots de passe ne sont jamais écrits** dans le Markdown, le front matter, l'archive, un journal ou un message d'erreur. Ils ne restent qu'en mémoire, effacés à la libération. Une macro enregistrée et Répéter ne les gardent pas. En ligne de commande, `passwordEnv` donne le nom d'une variable d'environnement, pour que le mot de passe n'apparaisse pas dans les arguments.
- **Aucune récupération n'est promise** : sans le mot de passe, personne, Plume compris, ne peut ouvrir le fichier. L'interface le dit.
- Les fichiers sont traités en mémoire, jusqu'à 1 Go.

### 1.3 Dans l'éditeur

- Ouvrir un `.md.aes`, un `.textpack.aes`, un `.zip.aes` ou un ZIP chiffré demande le mot de passe (même boîte que pour un document Word protégé). Enregistrer le réécrit chiffré avec le même mot de passe ; un ZIP garde ses autres fichiers.
- Enregistrer sous un nom en `.aes` sans mot de passe ouvre d'abord « Chiffrer avec un mot de passe », puis enregistre.
- La barre d'état montre un **cadenas** distinct de la classification : fermé et « Chiffré », ou ouvert et « Non chiffré ». Son menu dit la méthode, ce qui reste visible, et propose « Enregistrer une copie chiffrée… » ou « Modifier le mot de passe… ». Quand le niveau de classification demande le chiffrement (C4 par exemple), le cadenas ouvert passe à l'orange et le menu le dit ; la couleur n'est jamais le seul signal.
- Fichier › Informations : pour un fichier Markdown, « Enregistrer une copie chiffrée… » (un `.md` ne se chiffre pas sur place).
- Enregistrer sous (web) propose « Markdown, chiffré (*.md.aes) » et « TextPack, chiffré (*.textpack.aes) ».
- Publier sur une forge un document ouvert chiffré demande une **confirmation explicite** : la copie publiée est lisible (`acknowledged: true` pour la CLI et le MCP).

### 1.4 Commandes

| Commande | Rôle |
|---|---|
| `transport.convert` | `{from, to, password? \| passwordEnv?, sourcePassword? \| sourcePasswordEnv?, overwrite?}` : convertit et, au besoin, protège ; rend la liste des fichiers, des liens distants et ce qui reste visible |
| `crypto.encryptFile` | tout fichier en AES Crypt, vérifié avant écriture |
| `crypto.decryptFile` | un `.aes` ou un ZIP chiffré ; rien n'est écrit si le mot de passe est faux ou le fichier modifié |
| `crypto.inspect` | comment un fichier est protégé, sans mot de passe |

Aucune destination existante n'est écrasée sans `overwrite: true`, et tous les chemins sont vérifiés avant la première écriture.

## 2. Reste à faire

1. **age** (destinataires X25519 et phrase de passe) : lecture et écriture, pour chiffrer pour plusieurs personnes sans partager de mot de passe.
2. **7z AES-256** avec noms de fichiers chiffrés.
3. **Boîte Exporter unifiée** : format (md + ressources, ZIP, TextPack), protection (aucune, AES Crypt, age), destination (disque, dépôt Git), avec le résultat affiché (`document.textpack.aes`).
4. **Déploiement** vers un dossier, un dépôt Git local, GitHub, GitLab et Forgejo (dépôt, branche, dossier, contrôle des collisions), sans jamais déchiffrer pour publier sans validation.
5. **Politique par niveau** appliquée à l'export (C4 : export non chiffré refusé ou confirmé), au-delà de l'avertissement actuel.
6. Téléchargement explicite, sur demande, des ressources distantes listées.

## 3. Note juridique (vérifiée le 2026-10-21, sur sources secondaires)

Ce qui est établi :

- **Utiliser est libre** (LCEN, loi n° 2004-575, art. 30-I).
- **Fournir, importer, transférer dans l'UE ou exporter** un moyen de cryptologie relève par principe d'une **déclaration** auprès de l'ANSSI, parfois d'une autorisation (LCEN art. 30-III et 30-IV ; décret n° 2007-663). La démarche incombe au **fournisseur** (ou au premier importateur). Pour Plume, la question vise donc celui qui le distribue.
- **Dispense** : seuls les moyens listés à l'**annexe 1** du décret 2007-663 en sont dispensés. Les sources consultées ne citent **aucune dispense générale pour le logiciel libre ou gratuit**. Debian, par exemple, a déclaré sa distribution à l'ANSSI.
- **Dossier** (arrêté du 29 janvier 2015) : un formulaire et un dossier technique, avec le **code source** ou les moyens de le recompiler, et les composants qui chiffrent (ici `aes`, `cbc`, `pbkdf2`, `hmac`, `sha2`, et la crate `zip` pour ZIP AES). L'ANSSI répond **sous un mois** pour une déclaration complète.
- **Sanction** de la fourniture sans déclaration (LCEN art. 35) : jusqu'à un an d'emprisonnement et 15 000 € d'amende, selon les sources secondaires.

Ce qui reste à confirmer par le propriétaire, sur les textes officiels :

- la liste exacte de l'annexe 1, pour voir si une catégorie couvre le chiffrement de fichiers de Plume (c'est peu probable : AES-256 pour la confidentialité, clé choisie par l'utilisateur) ;
- qui est « fournisseur » quand Plume est publié sur une forge et téléchargé librement, depuis la France ou l'étranger ;
- le régime d'exportation hors UE (règlement UE 2021/821 sur les biens à double usage, et sa note sur les logiciels du domaine public).

Légifrance et cyber.gouv.fr n'étaient pas joignables depuis l'environnement de l'agent. Ce qui précède vient de pages qui les citent, et ce n'est pas un avis juridique. Pour une diffusion publique, la page ANSSI « Contrôle réglementaire sur la cryptographie : démarches à accomplir » et, au besoin, un avocat font foi. Rien n'a été déclaré ni publié par l'agent.

Sources : [décret 2007-663 (Légifrance)](https://www.legifrance.gouv.fr/loda/id/JORFTEXT000000646995) · [arrêté du 29 janvier 2015](https://www.legifrance.gouv.fr/loda/id/JORFTEXT000030255024) · [ANSSI, démarches](https://cyber.gouv.fr/reglementation/reglementation-identite-confiance-numerique/controles-reglementaires-cryptographie/controle-moyen-de-cryptologie/controle-rglementaire-cryptographie-demarches/) · [Debian, attestation de déclaration](https://www.debian.org/legal/anssi.fr.html) · [Next, « une exception française »](https://next.ink/10408/103575-les-outils-chiffrement-face-a-declaration-a-anssi-exception-francaise/) · [Deleporte Wentz Avocat](https://www.deleporte-wentz-avocat.com/actualite-importation-et-exportation-de-moyens-de-cryptologie-une-liberte-qui-reste-encadree)

## Historique

| Date | Changement |
|---|---|
| 2026-10-21 | Chiffrement derrière l'option `encryption`, désactivée par défaut : aucun algorithme dans les builds distribués. |
| 2026-10-21 | Note juridique vérifiée : déclaration de fourniture, pas de dispense générale pour le logiciel libre trouvée, points à confirmer. |
| 2026-10-21 | Création : moteur de transport, AES Crypt, ZIP AES-256, ouverture et enregistrement chiffrés, cadenas, confirmation avant publication d'un document chiffré. |
