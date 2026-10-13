# Keep the category name visible on the Slot shelf screen

Show the category name continuously at its existing opacity instead of fading
it out. Preserve the temporary alphabet-jump letter hint and restore the
category name when that hint finishes.

Status: complete. Requested by the user.

Category labels use a constant opacity. The letter hint retains its animation;
when it expires, the app clears the letter texture and the frontend uploads the
category name again. Existing menu and gameplay visibility rules are retained.

Validation: formatting and whitespace checks passed, along with all 77 app
tests and the shelf-hints test. The existing letter-jump test now checks that
category opacity persists, the category returns after the hint and the same
letter can be shown again. The device bundle needs rebuilding before deployment.
