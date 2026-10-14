# 05 — Intégration Git et forges (GitHub, GitLab, Forgejo)

> Statut : conception (rien d'implémenté). Légende : **[code]** vérifié · **[doc]** documentation publique des forges · **[hypothèse]** à prototyper.

## 1. Exigences retenues

- Ouvrir **un seul fichier Markdown à la fois** dans un dépôt Git (local ou distant).
- Authentification **GitHub, GitLab et Forgejo** ; un document peut être ouvert par toute personne ayant accès au dépôt.
- Chaque « version » = **un commit avec un message** ; l'utilisateur ne voit jamais le mot « commit ».
- Voir les révisions des collègues : **qui a modifié quoi, quand, avec quel commentaire**.
- **Conserver le mécanisme de versions existant** (`file.versions`, en mémoire : `crates/engine/src/cmd/tools.rs:396-412`) en le **branchant sur Git** quand le document est dans un dépôt.
- Enregistrer ≠ créer une version ≠ synchroniser (§9.2 du cahier des charges).

## 2. Vocabulaire présenté

| Git | Interface |
|---|---|
| commit | **Enregistrer une version** (avec « Décrivez vos modifications ») |
| push | **Synchroniser** (automatique après « Enregistrer une version » si connecté) |
| pull / fetch | **Récupérer les modifications** |
| log du fichier | **Historique du document** |
| diff | **Comparer / Afficher les modifications** |
| branche | **Version de travail** |
| merge request / pull request | **Demander une relecture** |
| review | **Relecture** (commentaires, approbation) |
| merge | **Intégrer / Publier** |
| conflit | **Modifications à résoudre** |

## 3. Architecture

```text
ui-egui (L6) ── panneau Historique, dialogue Connexion, onglet Publication
     │ commandes vcs.* (CommandSpec, comme tout le reste)
engine (L4) ── VersionStore (trait) : list / read_at / save_version / sync / restore
     │                      ├── MemoryStore  (comportement actuel, documents hors dépôt)
     │                      ├── LocalGitStore (dépôt sur disque)          [desktop]
     │                      └── ForgeStore    (API REST, sans clone)      [desktop + Web]
crate `forge` (nouvelle, L2, sans réseau propre) ── GitHub / GitLab / Forgejo : requêtes + décodage JSON
     │ Transport HTTP injecté par l'application
apps ── natif : client HTTP Rust (ex. `ureq` + rustls) ; Web : `fetch` du navigateur (web-sys)
```

Respect des règles du dépôt : le moteur reste sans réseau ni egui (`cargo xtask layers`), l'accès réseau est un **service fourni par l'application** (comme le dialogue de fichiers aujourd'hui, `apps/wordcraft-web/src/web.rs`), et les commandes programmatiques (CLI, MCP) passent par les mêmes `CommandSpec`.

### 3.1 Pourquoi l'API REST des forges plutôt qu'un clone Git ?

Le besoin est **un fichier à la fois**. Les trois forges exposent tout ce qu'il faut pour un fichier sans cloner le dépôt **[doc]** :

| Besoin | GitHub (REST v3) | GitLab (API v4) | Forgejo (API compatible Gitea) |
|---|---|---|---|
| Lire le fichier (à une révision) | `GET /repos/{o}/{r}/contents/{path}?ref=` | `GET /projects/:id/repository/files/:path?ref=` | `GET /repos/{o}/{r}/contents/{path}?ref=` |
| Écrire = 1 commit | `PUT …/contents/{path}` (message, `sha` attendu) | `PUT …/repository/files/:path` (commit_message, `last_commit_id`) | `PUT …/contents/{path}` (message, `sha`) |
| Plusieurs fichiers en 1 commit (`.md` + images) | API Git Data (blobs, tree, commit, ref) | `POST …/repository/commits` (actions multiples) | `POST …/contents` (fichiers multiples) |
| Historique du fichier | `GET …/commits?path=` | `GET …/repository/commits?path=` | `GET …/commits?path=` |
| Commentaires de commit | `GET/POST …/commits/{sha}/comments` | `GET/POST …/commits/:sha/comments` | **[hypothèse]** à vérifier |
| Demande de relecture | Pull requests + reviews | Merge requests + notes/discussions | Pull requests + reviews |

Avantages : fonctionne **identiquement sur desktop et dans le navigateur** ; pas de Git installé chez l'utilisateur ; le contrôle de concurrence est natif (le `sha`/`last_commit_id` attendu fait échouer l'écriture si quelqu'un a modifié le fichier entre-temps → déclenche la résolution, §7).

Le **dépôt local** (desktop) reste nécessaire pour les utilisateurs qui travaillent hors ligne ou sur un serveur Git « nu » : `LocalGitStore`.

### 3.2 Git local : choix de la bibliothèque

| Option | Licence | WASM | Commit / historique | Push / pull | Avis |
|---|---|---|---|---|---|
| `gix` (gitoxide, Rust pur) | MIT/Apache | partiel (pas de réseau) | oui | fetch oui ; push **[hypothèse]** à vérifier sur la version retenue | **recommandé** pour l'historique, la lecture d'une révision et le commit |
| `git2` (libgit2) | GPL-2.0 avec exception de liaison | non | oui | oui | dépendance C, complique les builds Windows/macOS universels |
| Commande `git` en sous-processus | — | non | oui | oui, avec les identifiants système (SSH, gestionnaire d'identifiants) | **repli** pour push/pull quand `git` est installé |

Recommandation : `gix` pour toutes les opérations locales ; synchronisation via l'API de la forge si le remote est une forge connue, sinon via la commande `git` si elle est présente, sinon synchronisation indisponible (message clair).

## 4. Authentification

| Forge | Méthode préférée | Repli | Remarques |
|---|---|---|---|
| GitHub | OAuth **device flow** (code à saisir sur github.com) | jeton personnel à portée fine | le device flow ne répond pas aux appels CORS du navigateur **[doc]** → la version Web demande un jeton ou passe par un petit relais |
| GitLab (gitlab.com ou auto-hébergé) | OAuth (PKCE), device flow selon version **[hypothèse]** | jeton personnel | l'instance auto-hébergée doit déclarer l'application OAuth |
| Forgejo / Codeberg | OAuth2 + PKCE | jeton d'accès | CORS de l'API à activer sur l'instance (`[cors]`) pour la version Web |

Stockage des secrets :

- desktop : **trousseau du système** (macOS Keychain, Windows Credential Manager, Secret Service sous Linux) via une crate de trousseau ; jamais dans un fichier de configuration ni dans le dépôt ;
- Web : jeton en mémoire de session uniquement (option « se souvenir » explicite, avec avertissement) ;
- SSH : laissé à la commande `git` (agent SSH de l'utilisateur).

Le canal de contrôle et MCP **ne peuvent pas lire** les jetons (les commandes `vcs.*` reçoivent un jeton opaque géré par l'application).

## 5. Parcours utilisateur

**Ouvrir depuis une forge** : Fichier → Ouvrir → *Depuis un dépôt en ligne* → choisir la forge → se connecter → choisir dépôt, branche, fichier `.md` → le document s'ouvre (images de `assets/` chargées par l'API, en cache local).

**Enregistrer** (Ctrl+S) : écrit la copie de travail (fichier local, ou cache local + journal de récupération pour un document distant). **Pas de commit.**

**Enregistrer une version** (Révision → Versions, ou Fichier → Informations) : boîte « Décrivez vos modifications » → 1 commit contenant le `.md` et les images nouvelles → synchronisation automatique si connecté. Le message par défaut est proposé à partir du rapport de différences (« 3 paragraphes modifiés dans *Installation* »).

**Historique du document** (panneau latéral, reprend la liste de `file.versions`) :

```text
Historique — guide-utilisateur.md
● Alice  · aujourd'hui 10:12 · « Mise à jour des captures »      💬 2
● Bob    · hier 17:40       · « Relecture chapitre 3 »
● Alice  · 12 oct.          · « Version initiale importée de guide.docx »
[Afficher les modifications] [Comparer avec la version actuelle] [Restaurer]
```

**Afficher les modifications** : la version N-1 et la version N sont relues en `Document` puis passées à **`review.compare`** (déjà implémenté) : le résultat s'affiche comme un document avec suivi des modifications, coloré par auteur, avec **Accepter / Rejeter** fonctionnels. C'est la réutilisation la plus forte de WordCraft dans ce projet.

**Restaurer** : ouvre l'ancienne version comme document courant ; l'enregistrement d'une version crée un nouveau commit (on ne réécrit jamais l'historique).

## 6. Relecture et circuit de validation

Correspondance du circuit §10.3 avec les objets des forges (facultatif, activé par document ou par dépôt) :

| Étape | Objet forge |
|---|---|
| Brouillon | commits sur une branche de travail `relecture/<auteur>-<date>` |
| En révision | demande de fusion ouverte, relecteurs désignés |
| Modifications demandées | revue « changes requested » |
| Nouvelle révision | nouveaux commits sur la branche |
| Approuvé | revue « approved » |
| Publié | fusion dans la branche principale |

Les **commentaires de relecture** posés sur la forge (sur une ligne du `.md`) sont importés comme commentaires WordCraft ancrés au paragraphe correspondant (ligne → paragraphe via la position des blocs à l'export) ; les réponses faites dans WordCraft sont renvoyées sur la forge. Les commentaires **internes au fichier** (03 §6) fonctionnent, eux, avec n'importe quel Git, même sans forge.

## 7. Conflits (« modifications à résoudre »)

Quand la synchronisation échoue parce que le fichier a changé à distance :

1. relire la version distante et la version de base commune ;
2. `review.compare(base, distante)` → révisions de l'autre auteur, appliquées sur la copie locale ;
3. l'utilisateur accepte/rejette comme dans Word ; puis nouvelle version.

Pas de marqueurs `<<<<<<<` dans le texte, jamais. **[hypothèse]** : la fusion par paragraphe (`compare` + application) suffit pour les cas courants ; à valider sur des scénarios réels (09 §5).

## 8. Sauvegarde automatique et récupération

Indépendantes de Git : journal local dans le dossier de données de l'application (pas dans le dépôt), écrit de façon atomique toutes les quelques secondes quand le document est modifié ; proposé à la réouverture après un arrêt anormal. Aujourd'hui l'auto-enregistrement n'existe que pour les formats sans perte et seulement sur le fichier lui-même (`ui-egui/src/lib.rs:1775`) ; il faut l'autoriser pour `.md` une fois Markdown devenu format maître.

## 9. Sécurité

- Les contenus venant d'une forge sont **non fiables** : mêmes limites que les fichiers (tailles, nombre d'images, profondeur) ; chemins d'images confinés au dépôt.
- Aucun appel réseau sans action explicite de connexion ; l'édition, la conversion, le PDF et l'historique local fonctionnent hors ligne.
- Portées minimales des jetons (lecture/écriture du contenu du dépôt, pull requests).
