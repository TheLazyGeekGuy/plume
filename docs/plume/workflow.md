# Plume : workflow sur branche de travail

> **Date :** 2026-10-19 · **Statut :** livré pour GitHub, GitLab et Forgejo

## Principe

Dans un dépôt dont la branche `main` est verrouillée, chaque personne travaille sur sa propre branche, nommée `<login>-<dépôt>`. Ses versions y sont enregistrées. Quand le travail est prêt, Plume ouvre la demande de fusion vers `main`, que les administrateurs acceptent.

## Ce que fait l'administrateur

1. **Verrouiller `main` sur la forge.** Interdire les poussées directes et exiger une demande de fusion approuvée :
   - GitHub : Settings › Branches › règle de protection sur `main` ;
   - GitLab : Settings › Repository › Protected branches ;
   - Forgejo : Paramètres › Branches › protection.
2. **Poser `.plume/workflow.toml` sur `main`.** Comme `main` est verrouillée, seule une demande de fusion approuvée peut ensuite le modifier :

```toml
# Chaque personne travaille sur sa branche ; main reçoit les changements par demande de fusion.
required = true
target = "main"
branch = "{user}-{repo}"
```

| Clé | Rôle | Par défaut |
|---|---|---|
| `required` | `false` désactive le workflow sans retirer le fichier | `true` |
| `target` | La branche verrouillée qui reçoit les fusions | La branche sur laquelle le document est ouvert |
| `branch` | Le modèle du nom de branche. `{user}` est le login de la forge, `{repo}` le nom du dépôt, mis en minuscules, chaque autre caractère devenant `-` | `{user}-{repo}` |

## Ce que voit l'utilisateur

| Moment | Ce qui se passe |
|---|---|
| Ouvrir un document du dépôt | Plume l'ouvre sur `ada-docs`, créée depuis `main` si elle n'existe pas encore |
| Fichier absent de sa branche | Plume ouvre la copie de `main` et l'ajoute à la branche au premier enregistrement |
| Enregistrer une version | La version va sur sa branche ; `main` ne bouge pas. Le dialogue l'indique et propose « Demander la fusion dans main » |
| Une demande est déjà ouverte | Le dialogue le dit : les nouvelles versions entrent dans cette demande |
| Après la fusion | À la prochaine ouverture, la branche repart de `main`, si rien n'y a été enregistré depuis la fusion |

Pour un script ou un agent, le même circuit passe par les commandes :
- **`forge.open` :** le résultat contient `workflow` (branche cible, branche de travail, branche neuve ou non).
- **`vcs.status` :** indique le workflow et la demande ouverte.
- **`vcs.requestReview` :** ouvre la demande de fusion depuis la branche de travail.

## Limites

- **Pas de fusion inverse.** Si `main` avance pendant qu'une demande est ouverte, Plume n'intègre pas `main` dans la branche de travail. La forge signale alors un conflit éventuel dans la demande.
- **Repartir sans perte.** Une branche est remise sur `main` seulement quand sa dernière demande est fusionnée et que rien n'y a été enregistré après. Sinon elle reste telle quelle.
