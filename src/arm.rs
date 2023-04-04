use proc_macro::{TokenStream, TokenTree, Group, Punct, Ident};

use crate::{errors::CfgBoostError, ts::CfgBoostMacroSource, syntax::NEGATIVE_SYMBOL};

/// Target arm separator
pub(crate) const ARM_SEPARATOR : char = ',';

/// Target attribute => content separator
pub(crate) const CONTENT_SEPARATOR_0 : char = '=';
pub(crate) const CONTENT_SEPARATOR_1 : char = '>';

/// Wild card _ arm symbol
pub(crate) const WILDCARD_BRANCH : char = '_';
pub(crate) const WILDCARD_BRANCH_STR : &str = "_";


/// Enumeration of possible arm types
#[derive(Debug, Clone, Copy)]
pub(crate) enum TargetArmType {
    /// Normal arm with predicates.
    Normal,

    /// Wildcard arm at the end.
    Wildcard,
}

/// Struct used that contains an arm type, it's attributes and content.
#[derive(Clone)]
pub(crate) struct TargetArm {
    pub arm_type : TargetArmType,
    pub attr : TokenStream,
    pub content : TokenStream,
}

impl ToString for TargetArm {
    /// Transform self into string.
    fn to_string(&self) -> String {
        format!("arm_type : {:?}, attr : {}, content : {}", self.arm_type, self.attr.to_string(), self.content.to_string())
    }
}

impl TargetArm {
    /// Create a new empty normal arm.
    pub fn new() -> TargetArm {
        TargetArm { arm_type : TargetArmType::Normal, attr : TokenStream::new(), content : TokenStream::new() }
    }

    /// Extract target arms into a vector from macro source.
    /// 
    /// Panic
    /// Will panic if no Wildcard arm inserted.
    pub fn extract(source : TokenStream, macro_src : CfgBoostMacroSource) -> Vec<TargetArm> {

        // Vector of all arms
        let mut arms : Vec<TargetArm> = Vec::new();

        // Arm used to extract attr and content.
        let mut arm = TargetArm::new();

        // Separator flags
        let mut separator = (false, false);

        // Negative modifier used for doc
        let mut is_negative = false;

        // 1. Extract Tokens from source
        for token in source {
            match token.clone() {
                // Extract group TokenStream
                proc_macro::TokenTree::Group(grp) => Self::extract_group_ts(grp, &mut arm, &mut arms, token, &mut separator),

                // Extract punctuation Tokenstream
                proc_macro::TokenTree::Punct(punct) => Self::extract_punct_ts(punct, &mut is_negative, &mut arm, &mut arms, token, &mut separator),

                // Extract ident Tokenstream
                proc_macro::TokenTree::Ident(ident) => Self::extract_ident_ts(ident, &mut is_negative, &mut arm, &mut arms, token, &mut separator),

                // Anything else content to attr or content
                _ => Self::add_ts_to_arm(TokenStream::from(token), &mut arm, &arms, separator),
            }
        }

        // 2. Add last arm if it were not added (missing `,` at last entry is not an error.)
        if separator.0 && separator.1 {
            arms.push(arm);
        }

        // 3. Verify arms integrity before returning them
        Self::verify_arms_integrity(macro_src, &mut arms);

        // 4. Return arms vector
        arms

    }

    /// This function make sure arms are verified for error and set default documentation.
    #[inline(always)]
    fn verify_arms_integrity(macro_src : CfgBoostMacroSource, arms: &mut Vec<TargetArm>) {
        // Verify Wildcard arm according to macro source and that single macro has only 1 arm.
        match macro_src {
            CfgBoostMacroSource::TargetMacro => {
                if Self::has_wild_arm(&arms){  // Single macro doesn't accept wildcard arms!
                    panic!("{}", CfgBoostError::WildcardArmOnTarget.message(""));
                }

                // // If any arm is inside a function, panic!
                for arm in arms {
                    if Self::is_inside_function(arm) {  
                        panic!("{}", CfgBoostError::TargetInFunction.message(""));
                    }
                }
            },
            _ => {  
                if !Self::has_wild_arm(&arms){  // Make sure a wildcard arm is written.
                    panic!("{}", CfgBoostError::WildcardArmMissing.message(""));
                }
            }
        }

        

    }

    /// Returns true if arm of macro is inside a function.
    /// 
    /// This function tries to detect `let` and flow of control keywords to determine if inside or not.
    /// 
    /// Since accuracy isn't 100%, it isn't used to validate that match_cfg! is inside a function. Only to detect if target_cfg! is.
    #[inline(always)]
    fn is_inside_function(arm: &TargetArm) -> bool {

        for t in arm.content.clone() {
            match t {
                proc_macro::TokenTree::Ident(ident) => match ident.to_string().as_str() {
                    "let" | "if" | "else" | "loop" | "break" | "while" | "for" | "match" | "println" | "panic"   => return true,    // Those keyword are only found inside functions.
                    _ => {},
                },
                _ => {},
            }
        }

        false
    }

    /// Extract group tokens from Tokenstream
    #[inline(always)]
    fn extract_group_ts(grp : Group, arm : &mut TargetArm, arms : &mut Vec<TargetArm>, token : TokenTree, separator : &mut (bool, bool)) {
        match grp.delimiter() {
            proc_macro::Delimiter::Brace => {
                if arm.content.is_empty() { // Add group only if content is empty.
                    Self::add_ts_to_arm(grp.stream(), arm, &arms, *separator);  // Extract group stream
                } else {
                    Self::add_ts_to_arm(TokenStream::from(token),arm, &arms, *separator);
                }
            },
            _ => Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator),
        }             
    }

    /// Extract punctuation tokens from Tokenstream
    #[inline(always)]
    fn extract_punct_ts(punct : Punct, is_negative : &mut bool, arm : &mut TargetArm, arms : &mut Vec<TargetArm>, token : TokenTree, separator : &mut (bool, bool)) {
        match punct.as_char() {
            NEGATIVE_SYMBOL => {    // Negative modifier. Used only on left hand side
                if !(separator.0 || separator.1) {
                    *is_negative = true;
                }
                Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator);
            }
            ARM_SEPARATOR => {   
                // Add arm to vector.
                arms.push(arm.clone());

                // Reset arm
                *arm = TargetArm::new();

                // Reset separator
                *separator = (false, false);
            },
            CONTENT_SEPARATOR_0 => {
                if !arm.content.is_empty() { // Indicate a missing arm separator
                    panic!("{}", CfgBoostError::ArmSeparatorMissing.message(""));
                }
                separator.0 = true
            },
            CONTENT_SEPARATOR_1 => {
                if !arm.content.is_empty() { // Indicate a missing arm separator
                    panic!("{}", CfgBoostError::ArmSeparatorMissing.message(""));
                }
                match arm.arm_type {    // Panic if arm attr is empty.
                    TargetArmType::Normal => if arm.attr.is_empty(){
                        panic!("{}", CfgBoostError::EmptyArm.message(""));
                    },
                    _ => {},
                }
                separator.1 = true
            },
            // Add content to attr or content
            _ => Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator),

        }
    }

    /// Extract ident tokens from Tokenstream
    #[inline(always)]
    fn extract_ident_ts(ident : Ident, is_negative : &mut bool, arm : &mut TargetArm, arms : &mut Vec<TargetArm>, token : TokenTree, separator : &mut (bool, bool)) {
        if !(separator.0 || separator.1) {
            match ident.to_string().as_str() {
                WILDCARD_BRANCH_STR => {
                    if arm.attr.is_empty() {    // Branch is a wildcard.
                        arm.arm_type = TargetArmType::Wildcard;
                    } else {
                        Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator); // Add token to attr or content.
                    }
                },  

                _ => Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator), // Add token to attr or content.

            }
        } else {    // Add token to attr or content.
            Self::add_ts_to_arm(TokenStream::from(token), arm, &arms, *separator);
        }
        *is_negative = false;    // Consume negative flag
    }

    /// Add tokenstream to arm attr or content according to separator.
    #[inline(always)]
    fn add_ts_to_arm(token : TokenStream, arm : &mut TargetArm, arms : &Vec<TargetArm>, separator : (bool, bool)) {
        Self::validate_arms(&arms, separator);    // Validate arm structures for integrity.
        if separator.0 || separator.1 {  // Add to content
            arm.content.extend(TokenStream::from(token));
        } else {    // Add to attributes
            arm.attr.extend(TokenStream::from(token));
        }
    }

    /// Validate arms structure and panic! is any syntax error occurs.
    /// 
    /// Panic(s)
    /// Panic if Wildcard arm isn't last
    /// Panic if there is a missing separator between arms.
    /// Panic if arm separator => incorrect.
    #[inline(always)]
    fn validate_arms(arms : &Vec<TargetArm>, separator : (bool, bool)) {
        if Self::has_wild_arm(&arms) { // Panic since wildcard arm isn't last.
            panic!("{}", CfgBoostError::WildcardArmNotLast.message(""));
        }
        if separator.0 && !separator.1 {    // Separator syntax error.
            panic!("{}", CfgBoostError::ContentSeparatorError.message(""));
        }
        if !separator.0 && separator.1 {    // Separator syntax error.
            panic!("{}", CfgBoostError::ContentSeparatorError.message(""));
        }
    }

    /// Returns true if a Wild arm is in arms vector.
    #[inline(always)]
    fn has_wild_arm(arms : &Vec<TargetArm>) -> bool {

        for arm in arms {
            match arm.arm_type {
                TargetArmType::Wildcard => return true,
                _ => {},
            }
        }

        // If no match, return false
        false
    }


}