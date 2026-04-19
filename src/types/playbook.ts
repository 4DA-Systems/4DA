// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export interface PlaybookModule {
  id: string;
  title: string;
  description: string;
  lesson_count: number;
  is_free: boolean;
}

export interface PlaybookLesson {
  title: string;
  content: string;
}

export interface PlaybookContent {
  module_id: string;
  title: string;
  description: string;
  lessons: PlaybookLesson[];
  is_free: boolean;
}

export interface PlaybookProgress {
  modules: PlaybookModuleProgress[];
  overall_percentage: number;
}

export interface PlaybookModuleProgress {
  module_id: string;
  completed_lessons: number[];
  total_lessons: number;
  percentage: number;
}
