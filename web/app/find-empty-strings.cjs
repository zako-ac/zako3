const fs = require('fs');

function findEmptyStrings(filePath) {
  const content = fs.readFileSync(filePath, 'utf-8');
  let line = 1;
  let inString = false;
  let isEscaped = false;
  let lastQuoteIndex = -1;
  let pathStack = [];
  let lastKey = null;

  // Minimal state machine parser
  for (let i = 0; i < content.length; i++) {
    const char = content[i];

    if (char === '\n') {
      line++;
      continue;
    }

    if (char === '\\' && inString) {
      isEscaped = !isEscaped;
      continue;
    }

    if (char === '"' && !isEscaped) {
      if (inString) {
        // Closing quote
        inString = false;
        const stringContent = content.substring(lastQuoteIndex + 1, i);
        
        // Peek ahead to see if this string is a key (followed by :) or a value
        let j = i + 1;
        while (j < content.length && /\s/.test(content[j])) j++;
        
        if (content[j] === ':') {
          // This is a key
          lastKey = stringContent;
        } else {
          // This is a value
          if (stringContent === '') {
            const path = [...pathStack, lastKey].filter(x => x !== null).join('.');
            console.log(`Empty string value found at Path: ${path} | Line: ${line}`);
          }
          lastKey = null; // Reset for next pair
        }
      } else {
        // Opening quote
        inString = true;
        lastQuoteIndex = i;
      }
      isEscaped = false;
      continue;
    }

    if (char === '{') {
      if (lastKey !== null) pathStack.push(lastKey);
      lastKey = null;
    } else if (char === '}') {
      pathStack.pop();
    }

    isEscaped = false;
  }
}

const filePath = process.argv[2];
if (!filePath) {
  console.error('Usage: node find-empty-strings.cjs <file.json>');
  process.exit(1);
}

findEmptyStrings(filePath);
