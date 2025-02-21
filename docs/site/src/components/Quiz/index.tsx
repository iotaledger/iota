import React, { useState, useEffect } from 'react';
import clsx from "clsx";


export default function Quiz(_questions) {
  const { questions } = _questions;

  useEffect(() => {
    setTimeout(() => {
      reset();
    }, 1000);
  }, [questions]);

  const [currentQuestion, setCurrentQuestion] = useState(0);
  const [showScore, setShowScore] = useState(false);
  const [score, setScore] = useState(0);
  const [clicked, setClicked] = useState(-1);
  const [isAnswered, setIsAnswered] = useState(false);
  const [showComponent, setShowComponent] = React.useState(false);
  const onClick = () => setShowComponent(!showComponent)

  const handleAnswerOptionClick = (isCorrect, index) => {
    if (isAnswered) return;

    setClicked(index);
    setIsAnswered(true)

    if (isCorrect) {
      setScore(score + 1);
    }

    setTimeout(() => {
      const nextQuestion = currentQuestion + 1;
      if (nextQuestion < questions.length) {
        setCurrentQuestion(nextQuestion);
      } else {
        setShowScore(true);
      }
      setClicked(-1);
      setIsAnswered(false);
    }, 1000);
  };

  const reset = () => {
    setShowScore(false);
    setScore(0);
    setCurrentQuestion(0);
    setIsAnswered(false); 
  };

  const Quiz = () => (
    <div className='app'>
      {showScore ? (
        <div className='score-section'>
          <p>
            You scored {score} out of {questions.length}
          </p>
          <a
            className='button button--outline button--primary'
            onClick={() => reset()}
          >
            {'Replay'}
          </a>
        </div>
      ) : (
        <>
          <div className='card'>
            <div className={clsx("feedback-header-container")}>
                <div className={clsx("h3", "question-header")}>Question {currentQuestion + 1}/{questions.length}</div>
                <button className={clsx("h3", "feedback-close", "button")} onClick={onClick}>X</button>
            </div>
            <div className='card__body'>
              {questions[currentQuestion]?.questionText}
            </div>
            <div className='card__footer'>
              {questions[currentQuestion]?.answerOptions.map(
                (answerOption, index) => (
                  <button
                    className={`button button--block button--primary margin-bottom--xs   ${
                      clicked >= 0 && answerOption.isCorrect
                        ? 'button--success'
                        : ''
                    } ${
                      clicked === index && !answerOption.isCorrect
                        ? 'button--danger'
                        : ''
                      } ${isAnswered ? 'button--disabled' : ''}`}
                    key={'answer-' + index}
                    onClick={() =>
                      handleAnswerOptionClick(answerOption.isCorrect, index)
                    }
                  >
                    {answerOption.answerText}
                  </button>
                ),
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
  const HiddenQuiz  = () => (
      <button  className={clsx("button", "hiddenForm")} onClick={onClick}>Answer Quiz</button>
  );

  return showComponent ? <Quiz/> : <HiddenQuiz/>;
}
