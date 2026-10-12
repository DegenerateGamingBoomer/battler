# Showdown quirks followed on purpose

Pokémon Showdown wins every disagreement (spec §2), cartridge behavior included. Each case where
battler follows a Showdown behavior that is odd, or that its own mainline data does differently, is
recorded here with the file in `third_party/pokemon-showdown` that defines it (pinned commit
`3661ce40…`).

| Area | Showdown behavior | Where |
| --- | --- | --- |
| Stats | Stat points ignore IVs and level: non-HP `base + SP + 20`, HP `base + SP + 75`, then the nature, truncated. Only the `Level Clause Mod` rule restores the mainline formula. | `data/mods/champions/scripts.ts` `statModify` |
| PP | Every move's base PP is capped at 20, then max PP is `(pp / 5 + 1) * 4` (float division in JS, so 15 → 16). Moves with `noPPBoosts` keep their PP (Revival Blessing: 1). | `scripts.ts` `init`, `calculatePP` |
| Public HP | Percentages use `floor`, minimum 1, with `y`/`r` at exactly 20% and `g`/`y` at exactly 50% (decided by the exact HP). | `sim/pokemon.ts` `getHealth` |
| Sleep | `sample([2, 3, 3])` turns: a 2/3 chance of the 3-turn sleep. | `conditions.ts` `slp` |
| Freeze | At most 3 turns, 1/4 thaw chance per turn; the turn counter is checked before the thaw roll. | `conditions.ts` `frz` |
| Trick Room | Speed is negated with no 8192 wraparound. battler uses `65535 - speed` for the same ordering. | `scripts.ts` `getActionSpeed` |
| Mega Evolution | A fainted Mega Pokémon stays Mega. | `scripts.ts` `formeChange` |
| Rage Fist | `clearVolatile` resets `timesAttacked`, and a multi-hit move adds one per hit. | `scripts.ts` `clearVolatile`, `hitStepMoveHitLoop` |
| Sheer Force | Does not suppress `AfterMoveSecondary` events (Eject Button, Berserk, Anger Shell). Emergency Exit and Wimp Out are still suppressed. | `scripts.ts` `afterMoveSecondaryEvent`, `hitStepMoveHitLoop` |
| Parental Bond | A Parental Bond move that hits once does not log `-hitcount`. | `scripts.ts` `hitStepMoveHitLoop` |
| Eject Button | When it fires, it no longer clears the attacker's pending switch (a U-turn user still switches). | `items.ts` `ejectbutton` |
| Emergency Exit / Wimp Out | Does not clear other Pokémon's pending switches. | `abilities.ts` |
| Fake Out / First Impression | The move is unselectable once the user has made any move action since switching in. | `moves.ts` |
| Belch / Stuff Cheeks | Not disabled in the move menu; they fail on use without a berry. | `moves.ts` |
| Healer | Cures an adjacent ally with a 1/2 chance (mainline 3/10). | `abilities.ts` |
| Run Away | Also prevents all trapping. | `abilities.ts` |
| Unseen Fist | Contact moves go through Protect at 0.25x damage and log `-zbroken`. battler's existing `weakenthroughprotection` flag already behaves this way; only the log differs. | `abilities.ts` `unseenfist` |
| Curse (non-Ghost) | Fails when none of the three stat changes can apply. | `moves.ts` `curse` |
| Dragon Darts | When Ally Switch moves the user into the position it chose, the first dart hits the user and the second its ally. | `sim/battle.ts` `getTarget` (`smartTarget`) |
| Two-turn moves | The move is the user's last move from its charging turn on (Encore, Mirror Move, Instruct see it). | `sim/battle-actions.ts` `runMove` (`moveUsed`) |
