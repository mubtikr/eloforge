export interface User {
  id: string
  chesscom_username: string
  display_name: string | null
  country: string | null
  avatar_url: string | null
  title: string | null
  current_elo: number | null
  peak_elo: number | null
  created_at: string
}

export interface ChallengeStart {
  challenge_id: string
  username: string
  nonce: string
  expires_at: string
  instructions: string
}

export interface AuthResponse {
  user: User
}
