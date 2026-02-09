const suggestedInterests = [
  'Machine Learning', 'Rust', 'TypeScript', 'Web Development',
  'DevOps', 'Security', 'Startups', 'Open Source', 'AI/LLM',
  'Mobile Development', 'Cloud Infrastructure', 'Data Engineering',
];

interface InterestsStepProps {
  isAnimating: boolean;
  role: string;
  setRole: (role: string) => void;
  interests: string[];
  setInterests: React.Dispatch<React.SetStateAction<string[]>>;
  newInterest: string;
  setNewInterest: (val: string) => void;
  onSave: () => void;
  onBack: () => void;
}

export function InterestsStep({
  isAnimating,
  role,
  setRole,
  interests,
  setInterests,
  newInterest,
  setNewInterest,
  onSave,
  onBack,
}: InterestsStepProps) {
  const addInterest = () => {
    if (newInterest.trim() && !interests.includes(newInterest.trim())) {
      setInterests([...interests, newInterest.trim()]);
      setNewInterest('');
    }
  };

  const removeInterest = (interest: string) => {
    setInterests(interests.filter(i => i !== interest));
  };

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">Your Interests</h2>
      <p className="text-gray-400 mb-6 text-center">
        Help 4DA understand what to surface. This improves over time as you use the app.
      </p>

      <div className="space-y-5 bg-[#141414] p-6 rounded-lg mb-6">
        {/* Role - simplified */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">
            What do you do? <span className="text-gray-600">(optional)</span>
          </label>
          <input
            type="text"
            value={role}
            onChange={(e) => setRole(e.target.value)}
            placeholder="e.g., Software Engineer, Product Manager, Researcher"
            className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
          />
        </div>

        {/* Interests - improved */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">
            Topics you want to follow
          </label>

          {/* Selected interests first */}
          {interests.length > 0 && (
            <div className="flex flex-wrap gap-2 mb-3 p-3 bg-[#1F1F1F] rounded-lg border border-[#2A2A2A]">
              {interests.map((interest) => (
                <span
                  key={interest}
                  className="px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-full text-sm flex items-center gap-2 animate-in fade-in duration-200"
                >
                  {interest}
                  <button
                    onClick={() => removeInterest(interest)}
                    className="hover:text-white text-orange-400/70"
                  >
                    &times;
                  </button>
                </span>
              ))}
            </div>
          )}

          {/* Add custom interest */}
          <div className="flex gap-2 mb-4">
            <input
              type="text"
              value={newInterest}
              onChange={(e) => setNewInterest(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && addInterest()}
              placeholder="Type a topic and press Enter..."
              className="flex-1 px-4 py-2 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
            />
            <button
              onClick={addInterest}
              disabled={!newInterest.trim()}
              className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Add
            </button>
          </div>

          {/* Suggestions - categorized */}
          <div className="space-y-3">
            <p className="text-xs text-gray-500">Or quick-add popular topics:</p>
            <div className="flex flex-wrap gap-2">
              {suggestedInterests
                .filter((s) => !interests.includes(s))
                .slice(0, 10)
                .map((suggestion) => (
                  <button
                    key={suggestion}
                    onClick={() => setInterests([...interests, suggestion])}
                    className="px-3 py-1.5 bg-[#1F1F1F] text-gray-400 rounded-full text-sm hover:bg-[#2A2A2A] hover:text-white transition-all hover:scale-105"
                  >
                    + {suggestion}
                  </button>
                ))}
            </div>
          </div>
        </div>

        {/* Hint */}
        <p className="text-xs text-gray-500 text-center">
          Don&apos;t worry about being complete - 4DA learns from your feedback and activity.
        </p>
      </div>

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; Back
        </button>
        <div className="flex items-center gap-3">
          <button
            onClick={() => {
              setInterests([]);
              setRole('');
              onSave();
            }}
            className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
          >
            Skip for now
          </button>
          <button
            onClick={onSave}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
          >
            {interests.length > 0 || role ? 'Save & Finish' : 'Finish Setup'}
          </button>
        </div>
      </div>
    </div>
  );
}
