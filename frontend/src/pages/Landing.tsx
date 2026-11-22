import { useMutation } from '@tanstack/react-query'
import { ArrowRight, Check, Copy, ShieldCheck } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { Link, useNavigate } from 'react-router-dom'

import { api, getErrorMessage } from '../api/client'
import type { AuthResponse, ChallengeStart } from '../api/types'
import { useAuth } from '../hooks/useAuth'

const EVAL_POINTS = [
  50, 52, 55, 53, 58, 61, 59, 64, 68, 66, 72, 70, 75, 73, 79, 77, 82, 80, 86, 84, 88, 91, 89, 93,
  96, 94, 97, 95, 92, 88, 84, 76, 68, 61, 55, 49, 44, 40, 38, 35, 33, 30,
]

function evalPath(points: number[], width: number, height: number): string {
  const max = points.length - 1
  return points
    .map((value, index) => {
      const x = (index / max) * width
      const y = height - (value / 100) * height
      return `${index === 0 ? 'M' : 'L'}${x.toFixed(2)},${y.toFixed(2)}`
    })
    .join(' ')
}

function EvalIllustration() {
  const width = 560
  const height = 150
  const path = evalPath(EVAL_POINTS, width, height)
  return (
    <figure className="eval-figure" aria-label="Illustrative evaluation curve from a real game review">
      <div className="eval-figure__head">
        <div>
          <p className="ef-label">Match lab · illustrative</p>
          <p className="eval-figure__title">White +0.4 → −2.1 in five moves</p>
        </div>
        <span className="eval-figure__tag">WINNING_GAME_THROWN</span>
      </div>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Evaluation swaps from an advantage to a losing position at move 31">
        <defs>
          <linearGradient id="eval-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#b98a3e" stopOpacity="0.24" />
            <stop offset="100%" stopColor="#b98a3e" stopOpacity="0" />
          </linearGradient>
        </defs>
        <line x1="0" y1={height / 2} x2={width} y2={height / 2} stroke="#d3c5ad" strokeDasharray="3 5" />
        <path d={`${path} L${width},${height} L0,${height} Z`} fill="url(#eval-fill)" />
        <path d={path} fill="none" stroke="#3f5560" strokeWidth="1.8" />
        <rect x={width * 0.63} y={height * 0.28} width="5" height="5" fill="#a64a24" />
        <rect x={width * 0.73} y={height * 0.5} width="5" height="5" fill="#a64a24" />
        <text x="3" y="14" fill="#7c7160" fontSize="9" fontFamily="IBM Plex Mono">
          +4
        </text>
        <text x="3" y={height - 4} fill="#7c7160" fontSize="9" fontFamily="IBM Plex Mono">
          −4
        </text>
      </svg>
      <figcaption className="eval-figure__foot">
        <span className="ef-mono">31. … Kh8?</span>
        <span>Stockfish 17 · depth 20 · −2.10</span>
        <span>1m 42s left</span>
      </figcaption>
    </figure>
  )
}

function ConnectForm() {
  const navigate = useNavigate()
  const [username, setUsername] = useState('')
  const [challenge, setChallenge] = useState<ChallengeStart | null>(null)
  const [copied, setCopied] = useState(false)

  const start = useMutation({
    mutationFn: (name: string) =>
      api<ChallengeStart>('/api/auth/chesscom/start', {
        method: 'POST',
        body: JSON.stringify({ username: name }),
      }),
    onSuccess: (data) => setChallenge(data),
  })

  const verify = useMutation({
    mutationFn: () =>
      api<AuthResponse>('/api/auth/chesscom/verify', {
        method: 'POST',
        body: JSON.stringify({ challenge_id: challenge?.challenge_id }),
      }),
    onSuccess: () => navigate('/app'),
  })

  function handleStart(event: FormEvent) {
    event.preventDefault()
    const clean = username.trim()
    if (!clean) return
    start.mutate(clean)
  }

  async function copyNonce() {
    if (!challenge) return
    try {
      await navigator.clipboard.writeText(challenge.nonce)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1800)
    } catch {
      setCopied(false)
    }
  }

  if (!challenge) {
    return (
      <form className="connect" onSubmit={handleStart}>
        <p className="ef-label">Connect your account</p>
        <h2 className="connect__title">Bring your real games.</h2>
        <p className="connect__copy">
          Enter your Chess.com username. We will give you a short code to place in your profile — no
          password, no OAuth waiting list.
        </p>
        <label className="connect__field">
          <span className="ef-label">Chess.com username</span>
          <input
            className="ef-input"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            placeholder="e.g. hikaru"
            autoComplete="username"
            spellCheck={false}
            maxLength={50}
          />
        </label>
        <button className="ef-btn ef-btn--primary connect__submit" type="submit" disabled={start.isPending}>
          {start.isPending ? 'Creating code…' : 'Start verification'}
          {start.isPending ? null : <ArrowRight size={15} strokeWidth={1.8} />}
        </button>
        {start.error ? <p className="connect__error">{getErrorMessage(start.error)}</p> : null}
        <p className="connect__legal">
          Read-only access to public games. We never ask for your Chess.com password.
        </p>
      </form>
    )
  }

  return (
    <div className="connect">
      <p className="ef-label">Step 2 of 2 · prove ownership</p>
      <h2 className="connect__title">Place this code in your profile.</h2>
      <p className="connect__copy">
        Open{' '}
        <a
          href="https://www.chess.com/settings/profile"
          target="_blank"
          rel="noreferrer"
          className="connect__link"
        >
          Chess.com → Settings → Profile
        </a>
        , paste the code into <strong>Location</strong> or <strong>Name</strong>, save, then verify.
        The code expires in 10 minutes.
      </p>
      {challenge.profile ? (
        <div className="profile-preview">
          {challenge.profile.avatar ? (
            <img src={challenge.profile.avatar} alt="" className="profile-preview__avatar" />
          ) : (
            <span className="profile-preview__avatar profile-preview__avatar--fallback">
              {challenge.profile.username.slice(0, 1).toUpperCase()}
            </span>
          )}
          <div>
            <strong>{challenge.profile.name ?? challenge.profile.username}</strong>
            <span className="ef-mono">
              @{challenge.profile.username}
              {challenge.profile.rapid_rating ? ` · Rapid ${challenge.profile.rapid_rating}` : ''}
            </span>
          </div>
        </div>
      ) : null}
      <button className="nonce" type="button" onClick={copyNonce} aria-label="Copy verification code">
        <span className="nonce__value ef-mono">{challenge.nonce}</span>
        <span className="nonce__copy">
          {copied ? <Check size={16} /> : <Copy size={16} />}
          {copied ? 'Copied' : 'Copy'}
        </span>
      </button>
      <ol className="connect__steps">
        <li>
          <span>01</span> Open your Chess.com profile settings.
        </li>
        <li>
          <span>02</span> Paste <span className="ef-mono">{challenge.nonce}</span> into Location.
        </li>
        <li>
          <span>03</span> Save, then press verify below.
        </li>
      </ol>
      <button
        className="ef-btn ef-btn--primary connect__submit"
        type="button"
        onClick={() => verify.mutate()}
        disabled={verify.isPending}
      >
        {verify.isPending ? 'Checking profile…' : 'Verify ownership'}
        {verify.isPending ? null : <ShieldCheck size={15} strokeWidth={1.8} />}
      </button>
      <button
        className="ef-btn ef-btn--ghost connect__back"
        type="button"
        onClick={() => {
          setChallenge(null)
          verify.reset()
        }}
      >
        Use a different username
      </button>
      {verify.error ? <p className="connect__error">{getErrorMessage(verify.error)}</p> : null}
      <p className="connect__legal">
        You can remove the code immediately after verification. It is only used once.
      </p>
    </div>
  )
}

export default function Landing() {
  const { user } = useAuth()
  return (
    <div className="landing ef-grain">
      <header className="landing__nav">
        <div className="ef-shell landing__nav-inner">
          <Link className="wordmark" to="/">
            <span className="wordmark__mark" aria-hidden="true">
              EF
            </span>
            <span className="wordmark__text">EloForge</span>
          </Link>
          <nav className="landing__links" aria-label="Primary">
            <a href="#method">Method</a>
            <a href="#reports">Reports</a>
            <a href="#ladder">Road to 1000</a>
          </nav>
          <Link className="ef-btn ef-btn--ghost landing__cta" to={user ? '/app' : '/connect'}>
            {user ? 'Open study room' : 'Connect Chess.com'}
          </Link>
        </div>
      </header>

      <main>
        <section className="ef-shell hero">
          <div className="hero__copy ef-fade-in">
            <span className="hero__rule" aria-hidden="true" />
            <h1 className="hero__title">
              Your games already
              <br />
              tell you why
              <br />
              you lose.
            </h1>
            <p className="hero__lede">
              EloForge reads every move with Stockfish 17, names the exact habit behind the result,
              and turns your own blunders into the drill you do tomorrow.
            </p>
            <div className="hero__actions">
              <a className="ef-btn ef-btn--primary" href="#connect">
                Start with your username
                <ArrowRight size={15} strokeWidth={1.8} />
              </a>
              <a className="hero__secondary" href="#method">
                See the method
              </a>
            </div>
            <dl className="hero__facts">
              <div>
                <dt className="ef-label">Engine</dt>
                <dd className="ef-mono">Stockfish 17</dd>
              </div>
              <div>
                <dt className="ef-label">Judgement</dt>
                <dd>Deterministic</dd>
              </div>
              <div>
                <dt className="ef-label">Coach</dt>
                <dd className="ef-mono">DeepSeek V4.1 Flash</dd>
              </div>
            </dl>
          </div>

          <div className="hero__side ef-fade-in" id="connect">
            <div className="hero__evidence">
              <EvalIllustration />
            </div>
            {user ? (
              <div className="connect connect--connected">
                <p className="ef-label">Account connected</p>
                <h2 className="connect__title">
                  {user.display_name ?? user.chesscom_username}
                </h2>
                <p className="connect__copy">
                  {user.current_elo ? `Rapid ${user.current_elo}. ` : ''}
                  Your games, drills and coach memory are waiting in the study room.
                </p>
                <Link className="ef-btn ef-btn--primary connect__submit" to="/app">
                  Open today's study room
                  <ArrowRight size={15} strokeWidth={1.8} />
                </Link>
              </div>
            ) : (
              <ConnectForm />
            )}
          </div>
        </section>

        <section className="method" id="method">
          <div className="ef-shell">
            <div className="method__head">
              <h2>
                Facts first.
                <br />
                Interpretation second.
              </h2>
              <p>
                A language model never decides whether a move is a blunder. Stockfish and the Rust
                analyzer do. The coach only explains the evidence that is already on the board.
              </p>
            </div>
            <ol className="method__steps">
              <li>
                <span className="method__num ef-mono">01</span>
                <div>
                  <h3>Every move, measured</h3>
                  <p>
                    Win probability, expected points lost, phase accuracy, clock spent, opening book
                    depth — computed in Rust before any AI request.
                  </p>
                </div>
              </li>
              <li>
                <span className="method__num ef-mono">02</span>
                <div>
                  <h3>Habits, not adjectives</h3>
                  <p>
                    Hanging pieces, missed mates, thrown winning positions, time-scramble blunders.
                    Tags come from deterministic rules, not a model's guess.
                  </p>
                </div>
              </li>
              <li>
                <span className="method__num ef-mono">03</span>
                <div>
                  <h3>Drills made from your losses</h3>
                  <p>
                    Each blunder becomes a puzzle from the position before you erred. Thrown endgames
                    reopen against a rating-calibrated Stockfish.
                  </p>
                </div>
              </li>
            </ol>
          </div>
        </section>

        <section className="reports" id="reports">
          <div className="ef-shell reports__inner">
            <div className="reports__copy">
              <h2>Read the game like a scorebook, not a scoreboard.</h2>
              <p>
                Eight reports, one evidence trail: phase accuracy, opening performance, blunder
                taxonomy, conversion rate, tilt curve, reflection history and the road ahead — all
                built from the same verified data.
              </p>
              <Link className="ef-btn ef-btn--ghost" to={user ? '/app' : '/connect'}>
                {user ? 'Open your reports' : 'Connect to see yours'}
              </Link>
            </div>
            <ul className="reports__list">
              <li>
                <span className="ef-mono">01</span> Weekly coach briefing
                <span className="reports__meta">AI + evidence</span>
              </li>
              <li>
                <span className="ef-mono">02</span> Match breakdown
                <span className="reports__meta">per game</span>
              </li>
              <li>
                <span className="ef-mono">03</span> Opening repertoire
                <span className="reports__meta">ECO A00–E99</span>
              </li>
              <li>
                <span className="ef-mono">04</span> Blunder taxonomy
                <span className="reports__meta">11 tags</span>
              </li>
              <li>
                <span className="ef-mono">05</span> Conversion &amp; endgames
                <span className="reports__meta">+3 / +5</span>
              </li>
              <li>
                <span className="ef-mono">06</span> Focus, speed &amp; tilt
                <span className="reports__meta">clock data</span>
              </li>
              <li>
                <span className="ef-mono">07</span> Reflection evolution
                <span className="reports__meta">your words</span>
              </li>
              <li>
                <span className="ef-mono">08</span> Road-to-GM progress
                <span className="reports__meta">8 stages</span>
              </li>
            </ul>
          </div>
        </section>

        <section className="ladder" id="ladder">
          <div className="ef-shell ladder__inner">
            <div className="ladder__cta">
              <h2>500 → 2500 is not one jump. It is nine unlocked doors.</h2>
              <p>
                Stage 1 is not opening theory. It is stopping free pieces. EloForge will not prescribe
                a Sicilian until the evidence says you have earned it.
              </p>
              <Link className="ef-btn ef-btn--primary" to="/connect">
                Put your games on the ladder
                <ArrowRight size={15} strokeWidth={1.8} />
              </Link>
            </div>
            <ol className="ladder__rail" aria-label="Rating stages">
              {[500, 650, 800, 1000, 1200, 1500, 1800, 2200, 2500].map((rating, index) => (
                <li key={rating} className={index < 3 ? 'is-done' : index === 3 ? 'is-next' : ''}>
                  <span className="ladder__dot" />
                  <span className="ladder__rating ef-mono">{rating}</span>
                  <span className="ladder__label">
                    {index === 0
                      ? 'Stop giving away pieces'
                      : index === 1
                        ? 'Tactical radar & king safety'
                        : index === 2
                          ? 'Conversion & core repertoire'
                          : index === 3
                            ? 'Structure & plans'
                            : index === 4
                              ? 'Calculation'
                              : index === 5
                                ? 'Repertoire identity'
                                : index === 6
                                  ? 'Prophylaxis'
                                  : index === 7
                                    ? 'Mastery'
                                    : 'Grandmaster ladder'}
                  </span>
                </li>
              ))}
            </ol>
          </div>
        </section>
      </main>

      <footer className="landing__footer">
        <div className="ef-shell landing__footer-inner">
          <div>
            <span className="wordmark__text">EloForge</span>
            <p>Independent training tool. Not affiliated with or endorsed by Chess.com.</p>
          </div>
          <p className="ef-mono">Built with Rust · Stockfish 17 · shakmaty</p>
        </div>
      </footer>
    </div>
  )
}
