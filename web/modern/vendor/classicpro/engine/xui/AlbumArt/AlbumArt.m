#include <lib/std.mi>
#include "../../lib/ClassicProFile.mi"

Function refreshCover();
//Function String getMyPath();
Function String getMyFile();

Global Group XUIGroup;
Global AlbumArtLayer albumart;
Global PopUpMenu popMenu;
Global Timer lookagain;


System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	albumart = XUIGroup.findObject("main.albumart");
	lookagain = new Timer;
	lookagain.setDelay(1000);
}
System.onScriptUnloading(){
	delete lookagain;
}

albumart.onRightButtonUp(int x, int y){
	popMenu = new PopUpMenu;

	if(System.getBuildNumber()<3235) popMenu.addCommand("Get Album Art", 1, 0, 0);
	popMenu.addCommand("Refresh Album Art", 2, 0, 0);
	popMenu.addCommand("Open Folder", 3, 0, 0);

	int result = popMenu.popAtMouse();

	if (result==1){
		if (system.getAlbumArt(system.getPlayItemString()) > 0)	{
			refreshCover();
		}
	}
	else if(result == 2){
		refreshCover();
	}
	else if (result == 3){
		//System.navigateUrl(getMyPath());
		ClassicProFile.exploreFile(getMyFile());
	}

	delete popMenu;
	complete;
}

albumart.onLeftButtonDblClk (int x, int y){
	albumart.setTargetA(150);
	albumart.setTargetSpeed(0.2);
	albumart.gotoTarget();
	//System.navigateUrl(getMyPath());
	ClassicProFile.exploreFile(getMyFile());
	//ClassicProFile.exploreFile("D:\# Music\Triphop nu jazz\Saint Germain - Des-Prés Café, Vol. 6\08-Horne Singers-Flat Foot.wma");

	/*
	test
	System.downloadMedia("http://www.albumartexchange.com/gallery/images/public/me/melody-melody_05.jpg", "D:\ ", 0, 0);	
	System.downloadURL("http://www.albumartexchange.com/gallery/images/public/me/melody-melody_05.jpg", "D:\cover.jpg", "Laai af");
*/
}

albumart.onTargetReached(){
	if(albumart.getAlpha()!=253){
		albumart.setTargetA(253);
		albumart.setTargetSpeed(0.6);
		albumart.gotoTarget();
	}
}

System.onTitleChange (String newtitle){
	refreshCover();
}

refreshCover(){
	// ---------- Stream info ----------
	String stype = getPlayItemMetaDataString("streamtype"); //"streamtype" will return "2" for SHOUTcast and "3" for AOL Radio
	
	if(strlower(getExtension(getPlayItemString()))=="nsv" && strleft(getPlayItemString(), 7) == "http://" && stype =="") stype = "sc_tv";

	if (stype == "2" || stype == "5"){
		albumart.setXmlParam("notfoundimage","cover.sc");
	}
	else if (stype == "3"){
		albumart.setXmlParam("notfoundimage", "cover.aolr");
	}
	else if (stype == "sc_tv"){
		Map myMap = new Map;
		myMap.loadMap("cover.sctv");
		
		if(myMap.getWidth()>64) albumart.setXmlParam("notfoundimage", "cover.sctv");
		else albumart.setXmlParam("notfoundimage", "cover.notfound");
		
		delete myMap;
	}
	else{
		albumart.setXmlParam("notfoundimage","cover.notfound");
	}

	AlbumArt.refresh();

	
	if(stype == "" && !lookagain.isRunning()) lookagain.start();
	else if(stype != "") lookagain.stop();
	
	//String stype2 = getPlayItemMetaDataString("contenttype"); //"streamtype" will return "2" for SHOUTcast and "3" for AOL Radio
	//setClipboardText(getPlayItemString());
}

lookagain.onTimer(){
	refreshCover();
}

/*String getMyPath() {
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPath(getPlayItemMetaDataString("filename"));
	return output;
}*/

String getMyFile() {
//debug(getPlayItemMetaDataString("filename"));
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");

//debug(output);

	return output;
}