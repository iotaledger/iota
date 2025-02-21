import React, {useState} from 'react';
import clsx from "clsx";
import "./styles.css";

const FeedbackForm = () => {
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');

  const [showComponent, setShowComponent] = useState(false);
  const onClick = () => setShowComponent(!showComponent);
  
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const prefixedTitle = `Docs feedback: ${title}`;
    const githubNewIssueUrl = `https://github.com/iotaledger/devx/issues/new?template=doc-bug.md&title=${encodeURIComponent(prefixedTitle)}&body=${encodeURIComponent(body)}`;

    // Open the GitHub issue page with pre-filled data in a new tab
    window.open(githubNewIssueUrl, '_blank');
    setTitle("");
    setBody("");
  };

  const VisibleFeedbackForm  = () => (
      <div className="feedback-container">
        <div className={clsx("feedback-header-container")}>
          <div className={clsx("h3", "feedback-header")}>Feedback Form</div>
          <button className={clsx("h3", "feedback-close", "button")} onClick={onClick}>X</button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label htmlFor="issue">Title <span className="red">*</span></label>
            <input
                type="text"
                id="title"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                required
                className="input-field"
                placeholder='Enter Title'
            />
          </div>
          <div className="form-group">
            <label htmlFor="body">Describe your feedback here</label>
            <textarea
                id="body"
                value={body}
                onChange={(e) => setBody(e.target.value)}
                required
                className="textarea-field"
                placeholder='Enter Text'
            />
          </div>
          <button
              className={clsx("button", { "button-disabled": !title })}
              type="submit"
              disabled={!title}
          >
            Submit Feedback
          </button>
        </form>
      </div>
  );

  const HiddenFeedbackForm  = () => (
   <button  className={clsx("button", "hiddenForm")} onClick={onClick}>Give Feedback</button>
  );

  return showComponent ? <VisibleFeedbackForm/> : <HiddenFeedbackForm/>;
};

export default FeedbackForm;
