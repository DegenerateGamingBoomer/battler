'use strict';
// Scenario definitions. Choices use Showdown syntax and are replayed on both engines.
// Every scenario brings the first four Pokémon of each team (`picks`), leading with the first two.
module.exports = set => {
  const S = [];
  const add = (name, description, teams, turns, extra = {}) =>
    S.push({name, description, teams, picks: ['1234', '1234'], turns, ...extra});
  const filler = (species = 'Blissey', ability = 'Natural Cure') => set(species, ability, ['splash']);

  add('basic-doubles', 'Leads, a turn of damaging moves with crits, a burn, a faint and a replacement.', [
    [set('Pikachu', 'Static', ['thunderbolt', 'protect']), set('Charizard', 'Blaze', ['flamethrower']),
      set('Gengar', 'Cursed Body', ['shadowball']), set('Snorlax', 'Immunity', ['tackle'])],
    [set('Snorlax', 'Immunity', ['tackle', 'swordsdance']), set('Gengar', 'Cursed Body', ['shadowball']),
      set('Pikachu', 'Static', ['thunderbolt']), set('Charizard', 'Blaze', ['flamethrower'])],
  ], [['move 1 1, move 1 1', 'move 1 1, move 1 1'], ['switch 3, pass', '']]);

  add('status-conditions', 'Paralysis, burn, poison, bad poison, sleep and freeze, with cures.', [
    [set('Pikachu', 'Static', ['thunderwave', 'splash']), set('Gengar', 'Cursed Body', ['toxic', 'willowisp', 'splash']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Snorlax', 'Immunity', ['splash', 'rest']), set('Blissey', 'Natural Cure', ['splash', 'healbell']),
      filler('Chansey', 'Natural Cure'), filler('Alakazam', 'Magic Guard')],
  ], [
    ['move 1 1, move 2 2', 'move 1, move 1'],
    ['move 2, move 1 1', 'move 1, move 1'],
    ['move 2, move 3', 'move 1, move 1'],
    ['move 2, move 3', 'move 2, move 2'],
    ['move 2, move 3', 'move 1, move 1'],
  ]);

  add('boosts', 'Boost, unboost, set boost, clear boosts and invert boosts.', [
    [set('Dragonite', 'Inner Focus', ['dragondance', 'bellydrum', 'splash']), set('Gyarados', 'Intimidate', ['splash']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Alakazam', 'Magic Guard', ['splash', 'haze', 'topsyturvy', 'clearsmog']), set('Machamp', 'Guts', ['bulkup', 'splash']),
      filler('Chansey', 'Natural Cure'), filler()],
  ], [
    ['move 1, move 1', 'move 1, move 1'],
    ['move 2, move 1', 'move 3 1, move 2'],
    ['move 1, move 1', 'move 4 1, move 1'],
    ['move 3, move 1', 'move 2, move 1'],
  ]);

  add('weather-field-sides', 'Weather, terrain, Trick Room, screens and hazards, started and ended.', [
    [set('Torkoal', 'Drought', ['reflect', 'lightscreen', 'splash']), set('Pelipper', 'Drizzle', ['splash', 'stealthrock', 'spikes']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Tyranitar', 'Sand Stream', ['grassyterrain', 'trickroom', 'splash', 'electricterrain']), set('Abomasnow', 'Snow Warning', ['splash', 'psychicterrain', 'mistyterrain']),
      filler('Chansey', 'Natural Cure'), filler()],
  ], [
    ['move 1, move 3', 'move 1, move 2'],
    ['move 2, move 2', 'move 3, move 1'],
    ['move 3, move 3', 'move 2, move 1'],
    ['move 3, move 1', 'move 4, move 1'],
    ['move 3, move 1', 'move 3, move 1'],
    ['move 3, move 1', 'move 3, move 1'],
    ['move 3, move 1', 'move 3, move 1'],
    ['move 3, move 1', 'move 3, move 1'],
  ]);

  add('protect-and-misses', 'Protect, repeated Protect failing, immunity, resistance and misses.', [
    [set('Gengar', 'Cursed Body', ['shadowball', 'protect', 'thunder']), set('Garchomp', 'Rough Skin', ['earthquake', 'protect', 'splash']),
      filler(), filler()],
    [set('Pikachu', 'Static', ['thunderbolt', 'protect', 'splash']), set('Skarmory', 'Sturdy', ['splash', 'protect']),
      filler(), filler()],
  ], [
    ['move 1 1, move 2', 'move 2, move 2'],
    ['move 1 1, move 2', 'move 2, move 2'],
    ['move 3 1, move 1', 'move 3, move 1'],
  ], {rng: 'max'});

  add('switching-and-healing', 'Dragging, Pain Split, Ally Switch, healing moves, Leftovers.', [
    [set('Dragonite', 'Multiscale', ['dragontail', 'roost', 'splash']), set('Alakazam', 'Magic Guard', ['allyswitch', 'painsplit', 'splash']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Tyranitar', 'Sand Stream', ['tackle', 'splash'], {item: 'Leftovers'}), set('Gengar', 'Cursed Body', ['shadowball', 'splash']),
      filler('Chansey', 'Natural Cure'), filler('Pikachu', 'Static')],
  ], [
    ['move 1 1, move 2 1', 'move 1 1, move 2'],
    ['move 2, move 1', 'move 1, move 1 1'],
    ['switch 3, pass', ''],
    ['move 1, move 3', 'move 1, move 1 2'],
  ]);

  add('mega-and-forms', 'Mega Evolution, details change and a stance forme change.', [
    [set('Charizard', 'Blaze', ['flamethrower', 'splash'], {item: 'Charizardite Y'}), set('Aegislash', 'Stance Change', ['kingsshield', 'shadowsneak', 'splash']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Gengar', 'Cursed Body', ['shadowball', 'splash'], {item: 'Gengarite'}), set('Garchomp', 'Rough Skin', ['earthquake', 'splash']),
      filler('Chansey', 'Natural Cure'), filler()],
  ], [
    ['move 1 1 mega, move 2 1', 'move 1 1 mega, move 2'],
    ['', 'switch 3, pass'],
    ['move 1 1, move 1', 'move 1, move 1'],
  ]);

  add('volatiles', 'Substitute, Taunt, Encore, Leech Seed, confusion, Yawn and their endings.', [
    [set('Gengar', 'Cursed Body', ['substitute', 'taunt', 'leechseed', 'splash']), set('Alakazam', 'Magic Guard', ['encore', 'confuseray', 'yawn', 'splash']),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Snorlax', 'Immunity', ['tackle', 'splash']), set('Blissey', 'Natural Cure', ['splash', 'tackle']),
      filler('Chansey', 'Natural Cure'), filler()],
  ], [
    ['move 1, move 1 1', 'move 1 1, move 1'],
    ['move 2 1, move 2 2', 'move 1 1, move 1'],
    ['move 3 2, move 4', 'move 1 1, move 1'],
    ['move 4, move 3 1', 'move 1 1, move 1'],
    ['move 4, move 4', 'move 1 1, move 1'],
    ['move 4, move 4', 'move 1 1, move 1'],
    ['move 4, move 4', 'move 1 1, move 1'],
  ]);

  add('items', 'Knock Off, Trick, berries, Air Balloon, White Herb and Spectral Thief.', [
    [set('Skarmory', 'Sturdy', ['knockoff', 'trick', 'splash'], {item: 'Leftovers'}), set('Gengar', 'Cursed Body', ['shadowball', 'splash', 'spectralthief'], {item: 'Air Balloon'}),
      filler(), filler('Snorlax', 'Immunity')],
    [set('Snorlax', 'Immunity', ['tackle', 'splash', 'closecombat'], {item: 'Sitrus Berry'}), set('Machamp', 'Guts', ['closecombat', 'bulkup', 'splash'], {item: 'White Herb'}),
      filler('Chansey', 'Natural Cure'), filler('Pikachu', 'Static')],
  ], [
    ['move 1 1, move 3 2', 'move 3 2, move 1 1'],
    ['move 2 2, move 1 1', 'move 1 1, move 2'],
    ['move 3, move 3 2', 'move 2, move 1 1'],
  ]);

  add('illusion', 'Illusion breaking replaces the displayed details.', [
    [set('Zoroark', 'Illusion', ['tackle', 'splash']), filler('Snorlax', 'Immunity'), filler(), filler('Chansey', 'Natural Cure')],
    [set('Gengar', 'Cursed Body', ['shadowball', 'splash']), set('Garchomp', 'Rough Skin', ['earthquake', 'splash']),
      filler('Chansey', 'Natural Cure'), filler()],
  ], [['move 1 1, move 1', 'move 1 1, move 2'], ['move 2, move 1', 'move 1 1, move 2']]);

  add('win', 'A side runs out of Pokémon.', [
    [set('Gengar', 'Cursed Body', ['hypervoice', 'splash']), set('Alakazam', 'Magic Guard', ['hypervoice', 'splash']),
      filler(), filler()],
    [set('Sunkern', 'Chlorophyll', ['splash']), set('Caterpie', 'Shield Dust', ['splash']),
      set('Magikarp', 'Swift Swim', ['splash']), set('Weedle', 'Shield Dust', ['splash'])],
  ], [['move 1, move 1', 'move 1, move 1'], ['', 'switch 3, switch 4'], ['move 1, move 1', 'move 1, move 1']]);

  add('simultaneous-faints', 'Both sides lose their last Pokémon in one move; the last to faint wins in gen 9.', [
    [set('Sunkern', 'Chlorophyll', ['explosion']), set('Caterpie', 'Shield Dust', ['explosion']),
      set('Magikarp', 'Swift Swim', ['explosion']), set('Weedle', 'Shield Dust', ['explosion'])],
    [set('Weedle', 'Shield Dust', ['explosion']), set('Magikarp', 'Swift Swim', ['explosion']),
      set('Caterpie', 'Shield Dust', ['explosion']), set('Sunkern', 'Chlorophyll', ['explosion'])],
  ], [['move 1, move 1', 'move 1, move 1'], ['switch 3, switch 4', 'switch 3, switch 4'], ['move 1, move 1', 'move 1, move 1']]);

  return S;
};
