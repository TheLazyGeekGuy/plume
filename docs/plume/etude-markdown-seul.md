# Plume, éditeur Markdown seul : étude

> **Date :** 2026-10-16 · **Statut :** décisions prises le 2026-10-16 (section 9), profil Plume livré · **Base mesurée :** branche `claude/gallant-allen-2rb6nx`, upstream `storytold/wordcraft` au commit `eae2421`

## 1. En bref

Le Markdown devient le seul format de travail. Word, DOC, RTF et ODT ne servent plus qu'à l'import, sans retour. L'export PDF passe par les modèles Eisvogel, à partir du Markdown.

Recommandation, en trois temps :

1. **Tout de suite, épurer sans supprimer.** Un profil Plume, actif par défaut, retire de l'interface et du registre des commandes tout ce que le Markdown ne peut pas garder. Le code reste dans le dépôt. Les fusions depuis WordCraft restent presque gratuites.
2. **Pendant un mois, mesurer.** Une fusion d'upstream par semaine, en notant le temps passé et les conflits. C'est la donnée qui manque pour décider de la suite.
3. **Ensuite, décider de la séparation.** Si les fusions coûtent peu, on garde le fork souple. Si upstream n'apporte plus grand-chose à Plume, on fait une séparation franche : suppression physique du code Office, renommage des crates, et reprise sélective des seuls correctifs d'import, de mise en page, de correction et de PDF.

Supprimer le code dès maintenant serait le choix le plus cher. Chaque commit d'upstream qui touche un fichier supprimé devient un conflit à résoudre à la main, et upstream modifie justement le plus souvent l'interface et le moteur de commandes.

## 2. Ce que montrent les mesures

### Le rythme d'upstream

| Mesure | Valeur |
|---|---|
| Commits d'upstream depuis le 16 septembre | 649, répartis sur 11 jours |
| Commits de Plume au-dessus d'upstream | 45 |
| Écart réel de contenu à fusionner aujourd'hui | 10 fichiers, 241 lignes ajoutées |
| Conflits d'une fusion aujourd'hui | 1, dans le PDF (`crates/pdf/src/lib.rs`) |

Upstream travaille par rafales : 128 commits le 10 octobre, 193 le 12, puis presque rien. Beaucoup de ses commits sont des fusions de branches dont le contenu était déjà là. L'écart réel reste donc faible si l'on fusionne souvent.

### Où upstream travaille

Fichiers touchés par upstream depuis le 16 septembre, par crate :

| Crate | Fichiers touchés | Utile à Plume |
|---|---|---|
| `ui-egui` (interface) | 645 | En partie : l'éditeur oui, les rubans Office non |
| `engine` (commandes) | 260 | En partie |
| `docx` (Word) | 153 | La lecture oui, l'écriture non |
| `layout` (mise en page) | 118 | Oui |
| `doc` (modèle) | 85 | Oui |
| `proof` (orthographe) | 71 | Oui |
| `docbin` (.doc) | 43 | Oui, pour l'import |
| `formats` (MD, HTML, RTF, ODT) | 39 | Lecture et Markdown oui |
| `pdf` | 15 | Oui |

La moitié de l'activité d'upstream porte sur l'interface. C'est aussi là que Plume veut le plus retirer de choses. C'est donc là qu'une suppression physique créerait le plus de conflits.

### Ce qui ne sert qu'à Office

Estimation par fichiers entiers, à affiner lors du tri :

| Zone | Lignes environ | Exemples |
|---|---|---|
| Écriture DOCX et Word 2003 XML | 6 700 | `crates/docx/src/write`, `wordml2003_write.rs` |
| Écritures RTF et ODT | 2 500 | `export` de `rtf.rs` et `odt.rs` |
| Commandes Office du moteur | 8 700 | publipostage, SmartArt, WordArt, graphiques, zone de dessin, encre, formes, protection, contrôles de contenu, mise en page, création, index |
| Dialogues Office de l'interface | 4 300 | graphiques, SmartArt, encre, rognage, mise en page, styles de tableau, onglet Équation |
| Total | **22 000 environ** | sur environ 250 000 lignes de Rust |

Le reste de l'interface est mêlé : un même fichier, comme le ruban ou les dialogues, sert les deux usages. Ce code-là ne se supprime pas par fichiers entiers, il se trie ligne par ligne.

Le mode strict Markdown, déjà livré, désactive 48 commandes sur 646. C'est un point de départ, pas le profil complet.

## 3. Les trois façons de neutraliser

| Méthode | Ce qu'on fait | Coût initial | Coût à chaque fusion | Réversible |
|---|---|---|---|---|
| **A. Profil à l'exécution** | Les commandes Office ne sont pas enregistrées et les onglets du ruban sont retirés, selon un profil Plume actif par défaut | 2 à 4 jours | Quasi nul : une nouvelle commande Office d'upstream apparaît, et il faut l'ajouter à la liste d'exclusion | Oui, en un réglage |
| **B. Exclusion à la compilation** | Des « features » Cargo excluent les modules Office du binaire | 1 à 2 semaines | Moyen : les listes d'enregistrement portent des `cfg`, et upstream modifie souvent ces mêmes lignes | Oui, en recompilant |
| **C. Suppression physique** | Les fichiers et modules Office quittent le dépôt | 2 à 3 semaines, tests compris | Fort : chaque commit d'upstream sur un fichier supprimé est un conflit « modifié ici, supprimé là » | Non, sauf à refusionner |

La méthode A donne déjà l'interface épurée que tu décris. L'utilisateur ne voit plus rien d'Office. Seuls le poids du binaire et le code mort restent.

Un garde-fou automatique rend la méthode A fiable dans le temps : un test qui échoue dès qu'une commande d'upstream n'est classée ni « Markdown » ni « Office ». Chaque fusion signale alors les nouveautés à trier, au lieu de les laisser passer en silence.

## 4. Stratégie de mise à jour depuis WordCraft

### Pendant le fork souple

- **Fusionner, ne pas trier commit par commit.** Avec 600 commits par quinzaine, la sélection commit par commit est intenable. Une fusion hebdomadaire d'`upstream/main` coûte peu tant que l'écart reste petit.
- **Garder le code Plume à part.** Le nouveau code Plume va dans ses propres modules ou crates, par exemple `crates/plume-*`. Les fichiers d'upstream ne reçoivent que des points d'accroche courts. Moins Plume modifie de lignes d'upstream, moins il y a de conflits.
- **Garder les noms d'upstream.** Les crates restent `wordcraft-*` tant qu'on fusionne. Le nom Plume se voit partout où l'utilisateur regarde, pas dans les chemins du code.
- **Une branche d'intégration.** La fusion se fait sur une branche dédiée, passe la CI Plume, puis rejoint `main`. Un conflit ne bloque jamais le travail courant.

### Après une séparation franche

- On ne fusionne plus `upstream/main`. On reprend seulement ce qui touche les zones utiles, filtrées par chemin : lecture DOCX, `.doc`, images WMF et EMF, mise en page, polices, orthographe, PDF, lecture HTML.
- La commande suivante liste ces commits à examiner :

```sh
git log --oneline LAST_SYNC..upstream/main -- crates/docx/src/read crates/docbin crates/metafile crates/layout crates/fonts crates/proof crates/pdf
```

- Chaque reprise est un « cherry-pick » ou un correctif réécrit. Il faut compter une demi-journée par semaine, selon l'activité d'upstream.

### Le critère pour passer de l'un à l'autre

On sépare franchement quand l'une de ces deux conditions est vraie pendant un mois :

- une fusion hebdomadaire prend plus d'une demi-journée ;
- moins d'un commit sur cinq d'upstream touche une zone utile à Plume.

## 5. Ce que Plume garde et ce qu'il retire

| Garder | Retirer de l'interface |
|---|---|
| Import DOCX, DOC, RTF, ODT, HTML, texte | Export DOCX, DOC, RTF, ODT |
| Lecture et écriture Markdown, profil natif sans HTML | Publipostage |
| Export PDF par modèles Eisvogel | SmartArt, WordArt, graphiques, zone de dessin, encre, formes |
| Titres, listes, tableaux simples, liens, notes, images | Zones de texte, lettrines, filigranes |
| Commentaires, en marqueurs HTML | En-têtes et pieds de page, sauts de page et de section, colonnes |
| Équations en LaTeX | Couleurs, soulignement, surlignage, bordures, trames |
| Versions, git local, forges GitHub, GitLab, Forgejo | Protection, contrôles de contenu, champs de formulaire |
| Restructuration des tableaux par IA | Mise en page de la feuille : marges, orientation, taille |
| Orthographe et grammaire | Styles de tableau et de page, thèmes Office |
| Recherche, plan, navigation | Macros et onglet Développeur |

Trois points restent à trancher, voir la section 8 : les citations Zotero, le suivi des modifications et l'index.

## 6. Le branding

### Déjà fait

Le nom affiché, l'identifiant d'application, les dossiers de réglages, l'icône, le README et le retrait des marques d'upstream sont en place depuis le 14 octobre. Le nom se règle à un seul endroit, `crates/brand`. Un test vérifie qu'aucun catalogue de traduction ne nomme encore WordCraft.

### Reste à faire

| Étape | Coût | Effet sur les fusions |
|---|---|---|
| Renommer le dépôt GitHub en `plume` | Quelques minutes, GitHub redirige les anciennes adresses | Aucun |
| Nommer les binaires `plume` et `plume-cli` | Une heure, sans renommer les crates | Faible |
| Champs `repository` et `description` des `Cargo.toml` | Une heure | Faible |
| README et NOTICE : « Plume est un projet distinct, né d'un fork de WordCraft » | Une heure | Faible, NOTICE change rarement upstream |
| Renommer les crates `wordcraft-*` en `plume-*` | Une journée | Fort : à faire seulement lors d'une séparation franche |

### Les obligations de licence

WordCraft est publié sous double licence MIT et Apache 2.0. Plume peut changer de nom, de logo et de communication. Il doit garder les fichiers de licence, les mentions de copyright d'origine et le fichier NOTICE, complété par ses propres mentions. Dire que Plume est un fork de WordCraft est donc à la fois une obligation et un bon message.

### Vérification du nom

Le nom « Plume » est déjà porté par d'autres logiciels, dont une plateforme de blog fédérée et des applications mobiles. Une recherche de marques et de noms de domaine est à faire avant toute communication publique.

## 7. Ce que Plume doit ajouter pour l'ergonomie

### Ouvrir et enregistrer dans un dépôt

- **Fichiers récents avec leur origine.** La liste des fichiers récents existe pour les fichiers locaux. Elle doit aussi retenir un fichier ouvert depuis une forge, avec le dépôt, la branche et le chemin, pour le rouvrir en un clic.
- **Dépôts récents.** Une liste des derniers dépôts ouverts, comme les dossiers récents d'Office.
- **Enregistrer, c'est committer.** Un enregistrement dans un dépôt propose le message de commit et la branche, sans quitter l'éditeur.
- **Statut visible.** La barre d'état indique le dépôt, la branche et l'état : à jour, modifié, en retard sur la forge.

### Le front matter, équivalent des propriétés Word

Plume lit et écrit déjà `title`, `author`, `subject`, `keywords`, `description` et la date de création. Il conserve aussi tel quel tout autre réglage, par exemple ceux d'Eisvogel. Il manque surtout la version, le statut et les champs que GitHub, VitePress et pandoc savent lire.

| Propriété Word | Clé du front matter | Lue par |
|---|---|---|
| Titre | `title` | pandoc, Eisvogel, VitePress, GitHub |
| Sujet | `subtitle` | pandoc, Eisvogel |
| Auteur | `author`, en liste s'il y en a plusieurs | pandoc, Eisvogel |
| Date | `date` | pandoc, Eisvogel |
| Dernière modification | `lastmod` | Hugo ; VitePress la tire de git |
| Mots clés | `keywords`, en liste | pandoc, Eisvogel |
| Commentaires et résumé | `description` ou `abstract` | VitePress, pandoc |
| Langue | `lang` | pandoc, Eisvogel |
| Numéro de révision | `version` | Plume, affiché dans Eisvogel par un modèle |
| Statut | `status` | Plume |
| Société | `publisher` ou `institute` | pandoc |
| Dernier modifié par | `editor` | Plume, à confirmer, voir la section 8 |

GitHub affiche le front matter sous forme de tableau en tête du fichier. Ces clés restent donc lisibles partout, sans HTML.

## 8. Questions pour toi

1. **Export Word.** Faut-il le supprimer complètement, ou garder un export DOCX simple pour envoyer un fichier à quelqu'un qui n'a que Word ?
2. **Citations Zotero.** Les garder ? Elles ont une forme Markdown, `[@clé]`, que pandoc et Eisvogel comprennent.
3. **Suivi des modifications.** Plume montre déjà les changements depuis la dernière version, à partir de git. Faut-il retirer le suivi des modifications de Word lui-même ?
4. **« Éditeur » dans l'en-tête.** S'agit-il de la personne qui a modifié le document en dernier, ou du logiciel qui l'a produit ?
5. **Calendrier.** D'accord pour commencer par le profil à l'exécution et un mois de mesures avant toute suppression physique ?
6. **Renommage du dépôt.** D'accord pour renommer `TheLazyGeekGuy/wordcraft` en `plume` maintenant ? C'est sans effet sur les fusions.

## 9. Décisions du 16 octobre 2026

| Question | Décision |
|---|---|
| Calendrier | Profil à l'exécution d'abord, un mois de mesures, aucune suppression physique avant |
| Export Word | Masqué par le profil, code conservé et réactivable |
| Citations Zotero | Gardées |
| Suivi des modifications | Le suivi Word est coupé par le profil. Le suivi « depuis la dernière version », appuyé sur git, reste |
| « Éditeur » dans l'en-tête | Les deux : `editor` pour la personne, `generator` pour le logiciel |
| Renommage du dépôt | Plus tard |
| Binaires `plume` et `plume-cli` | Plus tard |

### Ce que fait le profil livré

- **Classement des commandes.** Le fichier `crates/engine/src/plume_commands.txt` classe chacune des 662 commandes en `markdown` ou `office`. Un test échoue si une commande n'y figure pas, ou si le fichier nomme une commande disparue. Après une fusion, il dit quoi classer.
- **Commandes coupées.** Avec le profil, une commande `office` est refusée sur un document Markdown, par l'interface comme par la CLI et le MCP.
- **Interface épurée.** Les onglets Dessin, Création, Mise en page, Publipostage, Développeur, et les onglets contextuels Graphique, SmartArt et Format de la forme ne sont plus affichés. Les boutons et éléments de menu `office` des autres onglets ne sont plus dessinés.
- **Export.** Les exports DOCX, Word XML, Word 2003, ODT et RTF ne sont plus proposés. PDF, Markdown, HTML, LaTeX, texte et image restent.
- **Réglage.** Fichier › Options › Markdown › « S'en tenir au Markdown » désactive le profil et rend tout.

## 10. Journal des fusions depuis WordCraft

Une ligne par fusion, pour le critère de la section 4.

| Date | Commit upstream | Fichiers changés | Conflits | Temps passé | Commandes à classer | Remarque |
|---|---|---|---|---|---|---|
| 2026-10-16 | `eae2421` | 10 | 1 (`crates/pdf/src/lib.rs`, nom du créateur du PDF) | moins de 30 min | 0 | Nouveau moteur PDF partagé d'upstream ; Rust 1.95 requis |

