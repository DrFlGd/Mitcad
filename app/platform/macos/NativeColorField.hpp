// SPDX-License-Identifier: MIT
#pragma once

#import <AppKit/AppKit.h>

// Native presentation of the shared colour-field button. It keeps the
// presenter's button action, rather than using NSColorWell's own panel.
@interface MitcadNativeColorField : NSButton
@property(nonatomic, strong) NSColor* swatchColor;
@property(nonatomic, copy) NSString* words;
@property(nonatomic) BOOL placeholder;
- (NSRect)swatchRect;
- (NSRect)wordsRect;
@end
