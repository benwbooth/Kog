#include <lib/std.mi>

System.onScriptLoaded (){
	if(getPublicInt("ClassicPro.dontRemindOldWinamp", 0)==System.getDateDoy(System.getDate())) return;
	
	if(stringToInteger(getToken(getParam(), ";", 0))<=System.getBuildNumber()) return;
	
	int input = System.messageBox("For ClassicPro to work correctly, you'll need to update your Winamp!\n\nCurrent version installed:\tv"+System.getWinampVersion()+" (build " + integerToString(System.getBuildNumber()) +")\nMinimum Required:\t\tv"+getToken(getParam(), ";", 1)+" (build "+getToken(getParam(), ";", 0)+")\n\nDo you want to open the Winamp download page now?", "Please update your Winamp version!", 12, "");

	if(input==4){
		System.navigateUrl("http://www.winamp.com/player");
	}
	else{
		input = System.messageBox("Do you want to be reminded again today?\n\nNote: ClassicPro requires a new version because it makes use\n of certain new functions in the latest Winamp version.", "Please update your Winamp version!", 12, "");
		if(input==8) setPublicInt("ClassicPro.dontRemindOldWinamp", System.getDateDoy(System.getDate()));
		else setPublicInt("ClassicPro.dontRemindOldWinamp", 0);
	}
}