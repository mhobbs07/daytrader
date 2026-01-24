import React from 'react';
import { usePatternStore } from '../store/patternStore';

export const PatternList: React.FC = () => {
  const { patterns } = usePatternStore();

  return (
    <div className="pattern-list">
      <h3>Detected Patterns</h3>
      {patterns.length === 0 ? (
        <div>No patterns detected yet</div>
      ) : (
        <ul>
          {patterns.map((pattern, index) => (
            <li key={index} className={`pattern-${pattern.signal_type.toLowerCase()}`}>
              <strong>{pattern.pattern_name}</strong>
              {' '}{pattern.symbol}
              {' '}({pattern.signal_type})
              {' '}Confidence: {(pattern.confidence * 100).toFixed(1)}%
              <div className="pattern-time">
                {new Date(pattern.detected_at).toLocaleString()}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};