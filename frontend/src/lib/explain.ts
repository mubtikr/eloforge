import type { GameSummary, MoveDto } from '../api/types'

export interface MoveLesson {
  headline: string
  what: string
  why: string
  rule: string
  userMove: boolean
}

const TAG_WHY: Record<string, string> = {
  HANGING_PIECE:
    'The move left one of your pieces undefended, so the opponent could take it for free.',
  MISSED_FREE_PIECE:
    'A free capture was available, but the move chosen did not take it.',
  MISSED_MATE_1_2:
    'There was a forcing mate in one or two moves, and the winning pattern was missed.',
  ALLOWED_MATE:
    'The move opened a mating pattern for the opponent.',
  TACTICAL_BLINDNESS:
    'A short tactic — a fork, pin, skewer or double attack — decided the position.',
  WINNING_GAME_THROWN:
    'You had a winning advantage, but the position was not simplified before the counterplay arrived.',
  STALEMATE_THROW:
    'The winning endgame ended in stalemate because the opponent was left without a legal move.',
  ENDGAME_TECHNIQUE_FAIL:
    'The endgame needed a king improvement, rook cut or safe promotion, and the plan slipped.',
  IMPULSIVE_BLUNDER:
    'The move was played too quickly while plenty of clock remained.',
  TIME_SCRAMBLE_BLUNDER:
    'The position was played in the last seconds of the clock, where checks and captures become harder to see.',
  OPENING_PRINCIPLE_BREAK:
    'The move broke a basic opening principle: develop, castle, and do not move the same piece twice without a reason.',
}

const TAG_RULE: Record<string, string> = {
  HANGING_PIECE:
    'Before every move, name the undefended pieces on the board, then check whether your move leaves one of yours hanging.',
  MISSED_FREE_PIECE:
    'Before moving, list every free capture; if a safe one exists and no check is stronger, take it.',
  MISSED_MATE_1_2:
    'When you see a check, calculate it first: is there a forced mate in one or two?',
  ALLOWED_MATE:
    'After your candidate move, ask whether the opponent now has a forced mate.',
  TACTICAL_BLINDNESS:
    'Before moving, run the checks-captures-threats check for both sides.',
  WINNING_GAME_THROWN:
    'At +4 or better, simplify and trade pieces before looking for a second attack.',
  STALEMATE_THROW:
    'In a winning endgame, leave the opponent a legal move before every quiet move.',
  ENDGAME_TECHNIQUE_FAIL:
    'In a winning endgame, improve the king or rook before touching a pawn.',
  IMPULSIVE_BLUNDER:
    'If more than half your clock remains, spend at least four seconds on checks, captures and threats.',
  TIME_SCRAMBLE_BLUNDER:
    'Protect at least 30 seconds of clock; simplify before the scramble starts.',
  OPENING_PRINCIPLE_BREAK:
    'For the first ten moves, develop a new piece or castle; do not move the same piece twice without a concrete reason.',
}

const TAG_LABEL: Record<string, string> = {
  HANGING_PIECE: 'a hanging piece',
  MISSED_FREE_PIECE: 'a missed free piece',
  MISSED_MATE_1_2: 'a missed mate in one or two',
  ALLOWED_MATE: 'an allowed mate',
  TACTICAL_BLINDNESS: 'a missed tactic',
  WINNING_GAME_THROWN: 'a winning game thrown',
  STALEMATE_THROW: 'a stalemate throw',
  ENDGAME_TECHNIQUE_FAIL: 'an endgame technique failure',
  IMPULSIVE_BLUNDER: 'an impulsive move',
  TIME_SCRAMBLE_BLUNDER: 'a time-scramble error',
  OPENING_PRINCIPLE_BREAK: 'an opening-principle break',
}

function opponentName(game: GameSummary): string {
  return game.user_color === 'white' ? game.black_username : game.white_username
}

export function explainMove(move: MoveDto, game: GameSummary): MoveLesson {
  const userMove = move.color === game.user_color
  const who = userMove ? 'You' : opponentName(game)
  const pp = Math.max(0, (move.epl ?? 0) * 100)
  const best = move.best_move_san
  const playedIsBest = Boolean(best && move.best_move_uci === move.uci)
  const tagWhy = move.mistake_tags.map((tag) => TAG_WHY[tag]).find(Boolean)
  const tagRule = move.mistake_tags.map((tag) => TAG_RULE[tag]).find(Boolean)
  const tagSummary = move.mistake_tags
    .slice(0, 2)
    .map((tag) => TAG_LABEL[tag])
    .filter(Boolean)
    .join(' and ')

  const liveGame = ['rapid', 'blitz'].includes(game.time_class ?? '')
  const clockNote = liveGame && move.time_spent_sec !== null && move.time_spent_sec !== undefined
    ? move.time_spent_sec < 4
      ? ` You moved after only ${move.time_spent_sec.toFixed(1)} seconds.`
      : move.clock_remaining_sec !== null && move.clock_remaining_sec < 30
        ? ` You had ${move.clock_remaining_sec.toFixed(0)} seconds left.`
        : ''
    : ''

  const what = playedIsBest
    ? `${who} played ${move.san} on move ${move.move_number}; it was the engine's top choice. The position was already difficult, so the swing comes from the reply, not the move.${clockNote}`
    : `${who} played ${move.san} on move ${move.move_number}. It cost ${pp.toFixed(1)} percentage points of winning chances.${best ? ` The engine wanted ${best}.` : ''}${clockNote}`

  const why =
    tagWhy ??
    (tagSummary
      ? `The engine labels this as ${tagSummary}. The classification comes from the win-probability swing, the clock and the reply, not the final result.`
      : 'No narrower tag was attached. Step through the best line to see the resource the move missed — the engine swing is the evidence, not an opinion.')

  const rule =
    tagRule ??
    'Before moving, run the habit check: checks, captures, threats for both sides.'

  return {
    headline: `${move.san} · ${move.move_number}`,
    what,
    why,
    rule,
    userMove,
  }
}
