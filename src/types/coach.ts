// STREETS Coach types — mirrors src-tauri/src/streets_coach.rs

export interface CoachSession {
  id: string;
  session_type: string;
  title: string;
  context_snapshot: string | null;
  created_at: string;
  updated_at: string;
}

export interface CoachMessage {
  id: number;
  session_id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  token_count: number;
  cost_cents: number;
  created_at: string;
}

export interface EngineChoice {
  engine_number: number;
  engine_name: string;
  fit_score: number;
  time_to_first_dollar: string;
  revenue_range: string;
  reasoning: string;
  prerequisites: string[];
}

export interface EngineRecommendation {
  primary_engine: EngineChoice;
  secondary_engine: EngineChoice;
  reasoning: string;
  profile_gaps: string[];
}

export interface LaunchReviewResult {
  overall_score: number;
  strengths: string[];
  gaps: string[];
  recommendations: string[];
}

export interface CoachNudge {
  id: number;
  nudge_type: string;
  content: string;
  dismissed: boolean;
  created_at: string;
}

export interface CoachTemplate {
  id: string;
  title: string;
  description: string;
  category: string;
  content: string;
}

export interface VideoLesson {
  id: number;
  video_id: string;
  title: string;
  duration_seconds: number;
  drip_day: number;
  watched: boolean;
  watch_progress_seconds: number;
  unlocked: boolean;
  unlocked_at: string | null;
  watched_at: string | null;
}

export interface VideoCurriculumStatus {
  total_videos: number;
  unlocked_count: number;
  watched_count: number;
  total_duration_seconds: number;
  watched_duration_seconds: number;
  days_since_activation: number;
}

export type CoachSessionType = 'chat' | 'engine_recommender' | 'strategy' | 'launch_review' | 'progress' | 'templates' | 'curriculum';

export type StreetsTier = 'playbook' | 'community' | 'cohort';
